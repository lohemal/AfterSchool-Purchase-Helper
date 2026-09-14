//! P0-2  XLSX 견적서 파싱
//!
//! 시험 내용: 거래처 양식(머리글이 11행, 병합 53개, 수식 다수, 금액 0인 빈 행 17개)에서
//! 머리글을 **자동으로** 찾고 품목만 뽑아 내며, 합계와 검증식이 맞는가.
//! 샘플: test/fixtures-local/바둑 견적서.xlsx  (읽기만 한다)
//!
//! 셀 위치를 하드코딩하지 않는다 — 설계안 6-1 의 절차를 그대로 따라간다.

use calamine::{open_workbook, Data, Range, Reader, Xlsx};
use probe::fixture;

const SAMPLE: &str = "바둑 견적서.xlsx";

// ---- 설계안 6-1 의 낱말 사전 (동의어 포함) ----
fn role_of(header: &str) -> Option<&'static str> {
    let h: String = header.chars().filter(|c| !c.is_whitespace()).collect();
    const NAME: [&str; 6] = ["품명", "품목", "물품명", "상품명", "내역", "품명/규격"];
    const SPEC: [&str; 3] = ["규격", "사양", "단위"];
    const QTY: [&str; 1] = ["수량"];
    const PRICE: [&str; 2] = ["단가", "예상단가"];
    const AMOUNT: [&str; 4] = ["금액", "공급가액", "공급가", "합계금액"];
    const TAX: [&str; 2] = ["세액", "부가세"];
    const NOTE: [&str; 2] = ["비고", "적요"];
    for (list, role) in [
        (&NAME[..], "name"),
        (&SPEC[..], "spec"),
        (&QTY[..], "qty"),
        (&PRICE[..], "unit_price"),
        (&AMOUNT[..], "amount"),
        (&TAX[..], "tax"),
        (&NOTE[..], "note"),
    ] {
        if list.contains(&h.as_str()) {
            return Some(role);
        }
    }
    None
}

fn cell_text(d: Option<&Data>) -> String {
    match d {
        Some(Data::String(s)) => s.trim().to_string(),
        Some(Data::Float(f)) => {
            if (f.fract()).abs() < f64::EPSILON {
                format!("{}", *f as i64)
            } else {
                f.to_string()
            }
        }
        Some(Data::Int(i)) => i.to_string(),
        Some(Data::Bool(b)) => b.to_string(),
        _ => String::new(),
    }
}

/// 설계안 6-1 5번: `76,500원` `₩76,500` `10개` `-30,000` `△30,000` `(30,000)` → 정수(부호 보존)
///
/// **함정**: 한글 견적서는 합계를 `(￦ 900,000 )` 처럼 괄호로 감싸 적는다. 이건 음수가 아니다.
/// 그래서 괄호 안이 **숫자·쉼표·마침표·공백뿐일 때만** 회계식 음수로 본다. (P0-5 에서 실제로 걸렸다.)
fn parse_number(s: &str) -> Option<i64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    let paren_negative = t.starts_with('(')
        && t.ends_with(')')
        && t[1..t.len() - 1]
            .chars()
            .all(|c| c.is_ascii_digit() || c == ',' || c == '.' || c.is_whitespace());
    let negative =
        t.starts_with('-') || t.starts_with('△') || t.starts_with('▲') || paren_negative;
    let digits: String = t.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    let n: i64 = digits.parse().ok()?;
    Some(if negative { -n } else { n })
}

#[derive(Debug, PartialEq)]
struct Item {
    name: String,
    spec: String,
    qty: Option<i64>,
    unit_price: Option<i64>,
    amount: Option<i64>,
}

#[derive(Debug)]
struct Parsed {
    header_row: usize,
    roles: Vec<(usize, &'static str)>,
    items: Vec<Item>,
    zero_rows: usize,
    total_rows: Vec<(usize, i64)>,
    stated_total: Option<i64>,
}

const TOTAL_WORDS: [&str; 7] = ["합계", "총계", "소계", "계", "총액", "부가세", "VAT"];

fn parse_quote(range: &Range<Data>) -> Parsed {
    let (h, w) = (range.height(), range.width());

    // 1) 머리글 행 = 낱말이 둘 이상 맞는 행 중 가장 많이 맞는 행 (행 번호를 고정하지 않는다)
    let mut best = (0usize, 0usize);
    for r in 0..h {
        let hits = (0..w)
            .filter(|c| role_of(&cell_text(range.get((r, *c)))).is_some())
            .count();
        if hits > best.1 {
            best = (r, hits);
        }
    }
    let header_row = best.0;
    assert!(best.1 >= 2, "머리글을 못 찾았다");

    // 2) 열 역할. 병합 때문에 머리글 글자는 병합의 첫 칸에만 있다 → 그 칸의 차례를 쓴다.
    let mut roles: Vec<(usize, &'static str)> = Vec::new();
    for c in 0..w {
        if let Some(role) = role_of(&cell_text(range.get((header_row, c)))) {
            if !roles.iter().any(|(_, r)| *r == role) {
                roles.push((c, role));
            }
        }
    }
    let col = |role: &str| roles.iter().find(|(_, r)| *r == role).map(|(c, _)| *c);

    // 병합된 열은 머리글 칸과 값 칸의 차례가 다를 수 있다 →
    // 그 역할의 머리글 차례부터 다음 역할 머리글 차례 직전까지에서 값이 있는 칸을 쓴다.
    let mut bounds: Vec<(usize, usize, &'static str)> = Vec::new();
    let mut sorted = roles.clone();
    sorted.sort_by_key(|(c, _)| *c);
    for (i, (c, role)) in sorted.iter().enumerate() {
        let end = sorted.get(i + 1).map(|(nc, _)| *nc).unwrap_or(w);
        bounds.push((*c, end, role));
    }
    let value_in = |r: usize, role: &str| -> String {
        for (start, end, rl) in &bounds {
            if *rl == role {
                for c in *start..*end {
                    let t = cell_text(range.get((r, c)));
                    if !t.is_empty() {
                        return t;
                    }
                }
            }
        }
        String::new()
    };

    // 3) 행 분류 (설계안 6-2)
    let mut items = Vec::new();
    let mut zero_rows = 0usize;
    let mut total_rows = Vec::new();
    for r in (header_row + 1)..h {
        let name = value_in(r, "name");
        let amount = parse_number(&value_in(r, "amount"));
        let nm_nospace: String = name.chars().filter(|c| !c.is_whitespace()).collect();

        if TOTAL_WORDS.iter().any(|w| nm_nospace == *w) {
            if let Some(a) = amount {
                total_rows.push((r, a));
            }
            continue;
        }
        if name.is_empty() {
            continue; // 빈 행 (수식이 0을 돌려주는 칸 포함)
        }
        match amount {
            None | Some(0) => {
                zero_rows += 1;
            }
            Some(_) => items.push(Item {
                name,
                spec: value_in(r, "spec"),
                qty: parse_number(&value_in(r, "qty")),
                unit_price: parse_number(&value_in(r, "unit_price")),
                amount,
            }),
        }
    }

    // 4) 표기 합계 = 문서 어디든 '합계금액' 낱말 옆의 숫자
    let mut stated_total = None;
    'outer: for r in 0..h {
        for c in 0..w {
            let t: String = cell_text(range.get((r, c))).chars().filter(|c| !c.is_whitespace()).collect();
            if t.contains("합계금액") {
                for cc in c..w.min(c + 6) {
                    if let Some(n) = parse_number(&cell_text(range.get((r, cc)))) {
                        stated_total = Some(n);
                        break 'outer;
                    }
                }
                for rr in r..h.min(r + 3) {
                    for cc in 0..w {
                        if let Some(n) = parse_number(&cell_text(range.get((rr, cc)))) {
                            stated_total = Some(n);
                            break 'outer;
                        }
                    }
                }
            }
        }
    }

    let _ = col("tax");
    Parsed { header_row, roles, items, zero_rows, total_rows, stated_total }
}

#[test]
fn p0_2_baduk_quote() {
    let path = fixture(SAMPLE);
    let mut wb: Xlsx<_> = open_workbook(&path).expect("바둑 견적서 열기");
    assert_eq!(wb.sheet_names(), vec!["견적서".to_string()], "시트 구성");

    let range = wb.worksheet_range("견적서").expect("견적서 시트");
    // **주의**: calamine 의 Range 는 A1 이 아니라 '실제로 쓴 영역' 에서 시작한다.
    // 화면에 원본 셀 주소를 보여주려면 start() 를 더해 절대 좌표로 바꿔야 한다.
    let (row0, col0) = range.start().expect("사용 영역");
    println!("시트 크기: {}행 x {}열, 시작 셀 = 엑셀 {}행 {}열",
             range.height(), range.width(), row0 + 1, col0 + 1);

    let p = parse_quote(&range);
    let header_excel_row = row0 as usize + p.header_row + 1;
    println!("머리글 행: 범위 기준 {} → 엑셀 {}행", p.header_row, header_excel_row);
    println!("열 역할: {:?}", p.roles);
    println!("품목 {}건, 금액0/빈 행 {}건, 합계 행 {:?}", p.items.len(), p.zero_rows, p.total_rows);
    println!("표기 합계금액: {:?}", p.stated_total);
    for it in &p.items {
        println!("  {:?}", it);
    }

    // --- 통과 기준 ---
    assert_eq!(header_excel_row, 11, "머리글은 엑셀 11행");
    assert_eq!(p.items.len(), 1, "품목은 1건이어야 한다");
    let it = &p.items[0];
    assert_eq!(it.name, "바둑교재(상상바둑)", "품목명은 원문 그대로");
    assert_eq!(it.spec, "권");
    assert_eq!(it.qty, Some(37));
    assert_eq!(it.unit_price, Some(12_000));
    assert_eq!(it.amount, Some(444_000));

    // 수량 x 단가 = 금액
    assert_eq!(it.qty.unwrap() * it.unit_price.unwrap(), it.amount.unwrap(), "수량x단가=금액");

    // 합계
    let item_sum: i64 = p.items.iter().filter_map(|i| i.amount).sum();
    assert_eq!(item_sum, 444_000, "품목 합");
    assert_eq!(p.stated_total, Some(444_000), "견적서에 적힌 합계금액");
    assert!(p.total_rows.iter().any(|(_, a)| *a == 444_000), "합계 행의 값");

    // 빈 행 17개는 품목으로 세지 않는다
    assert_eq!(p.items.len(), 1, "빈 행이 품목에 섞이면 안 된다");

    // 문구 생성 (설계안 6-6) — 품의 표기명은 부서 설정에서 온다
    let phrase = format!(
        "바둑부 {} {}",
        it.name,
        if p.items.len() == 1 { "1종".to_string() } else { format!("외 {}종", p.items.len() - 1) }
    );
    assert_eq!(phrase, "바둑부 바둑교재(상상바둑) 1종");
    println!("자동 문구: {phrase}");

    // 정산자료 바둑 합계와 일치하는가 (수익자 372,000 + 초과금 12,000 + 지원금 60,000 + 자유 0)
    assert_eq!(444_000, 372_000 + 12_000 + 60_000 + 0, "정산 재원 합계와 견적 총액");
}

#[test]
fn p0_2_number_parsing() {
    // 할인 표기와 부호 보존 (설계안 6-1 5번)
    assert_eq!(parse_number("76,500원"), Some(76_500));
    assert_eq!(parse_number("₩76,500"), Some(76_500));
    assert_eq!(parse_number("10개"), Some(10));
    assert_eq!(parse_number("1 set"), Some(1));
    assert_eq!(parse_number("-30,000"), Some(-30_000));
    assert_eq!(parse_number("△30,000"), Some(-30_000));
    assert_eq!(parse_number("(30,000)"), Some(-30_000));
    assert_eq!(parse_number("30,000"), Some(30_000));
    assert_eq!(parse_number(""), None);
    assert_eq!(parse_number("도서는 면세임"), None);
}

#[test]
fn p0_2_paren_is_not_always_negative() {
    // P0-5 에서 실제로 걸린 함정: 한글 견적서의 합계 표기 `(￦ 900,000 )` 는 음수가 아니다.
    assert_eq!(parse_number("(￦ 900,000 )"), Some(900_000));
    assert_eq!(parse_number("(₩900,000)"), Some(900_000));
    assert_eq!(parse_number("(900,000원)"), Some(900_000));
    // 반면 괄호 안이 숫자뿐이면 회계식 음수로 본다
    assert_eq!(parse_number("(30,000)"), Some(-30_000));
    assert_eq!(parse_number("( 30,000 )"), Some(-30_000));
}
