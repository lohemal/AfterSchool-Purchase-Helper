//! P0-5  HWPX 표 파싱
//!
//! 시험 내용: P0-4 가 만든 HWPX 에서 **견적서 표를 골라**(납품서가 아니라) 품목을 뽑는다.
//! 병합이 많으므로 `cellAddr` 로 격자를 복원해야 한다 — 셀을 차례로 세면 어긋난다.
//! 샘플: test/out/p0-4-로봇과학.hwpx  (P0-4 를 먼저 돌려야 한다)
//!
//!   cargo test --test p0_5_hwpx_table -- --ignored --nocapture --test-threads=1

use probe::out_dir;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::io::Read;

#[derive(Debug, Clone)]
struct Cell {
    col: usize,
    row: usize,
    col_span: usize,
    text: String,
}

#[derive(Debug)]
struct Table {
    /// 표 바로 앞/안에서 찾은 제목 글자 (견적서 / 납품서 구분에 쓴다)
    title: String,
    rows: Vec<Vec<Cell>>,
}

fn local(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or("")
}

fn xml_text(t: &quick_xml::events::BytesText) -> String {
    let raw = t.xml_content(quick_xml::XmlVersion::Implicit1_0);
    quick_xml::escape::unescape(&raw).map(|c| c.to_string()).unwrap_or_else(|_| raw.to_string())
}

fn attr(e: &quick_xml::events::BytesStart, key: &str) -> Option<String> {
    e.attributes().flatten().find(|a| a.key.as_ref() == key).map(|a| a.value.to_string())
}

/// section XML 에서 표를 모두 뽑는다.
fn parse_tables(xml: &str) -> Vec<Table> {
    let mut r = Reader::from_str(xml);
    let mut tables: Vec<Table> = Vec::new();

    // 표 깊이 (표 안에 표가 있을 수 있다)
    let mut depth = 0usize;
    let mut cur: Option<Table> = None;
    let mut cur_row: Vec<Cell> = Vec::new();
    let mut cur_cell: Option<Cell> = None;
    let mut in_t = false;

    loop {
        let ev = r.read_event();
        match ev {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let is_empty = matches!(ev, Ok(Event::Empty(_)));
                match local(e.name().as_ref()) {
                    "tbl" => {
                        depth += 1;
                        if depth == 1 {
                            cur = Some(Table { title: String::new(), rows: Vec::new() });
                        }
                    }
                    "tr" if depth == 1 => cur_row = Vec::new(),
                    "tc" if depth == 1 => {
                        cur_cell = Some(Cell { col: 0, row: 0, col_span: 1, text: String::new() })
                    }
                    // 병합을 반영하려면 **cellAddr 로 격자 좌표를 읽어야** 한다.
                    // 셀을 차례로 세면 병합된 자리만큼 어긋난다.
                    "cellAddr" if depth == 1 => {
                        if let Some(c) = cur_cell.as_mut() {
                            c.col = attr(e, "colAddr").and_then(|v| v.parse().ok()).unwrap_or(0);
                            c.row = attr(e, "rowAddr").and_then(|v| v.parse().ok()).unwrap_or(0);
                        }
                    }
                    "cellSpan" if depth == 1 => {
                        if let Some(c) = cur_cell.as_mut() {
                            c.col_span =
                                attr(e, "colSpan").and_then(|v| v.parse().ok()).unwrap_or(1);
                        }
                    }
                    "t" => {
                        if !is_empty {
                            in_t = true
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref t)) => {
                let s = xml_text(t);
                if in_t {
                    if let Some(c) = cur_cell.as_mut() {
                        c.text.push_str(&s);
                    } else if depth == 0 {
                        // 표 밖의 글자 — 다음 표의 제목 후보로 쌓아 둔다
                        if let Some(tb) = cur.as_mut() {
                            tb.title.push_str(&s);
                        }
                    }
                }
            }
            Ok(Event::End(ref e)) => match local(e.name().as_ref()) {
                "t" => in_t = false,
                "tc" if depth == 1 => {
                    if let Some(c) = cur_cell.take() {
                        cur_row.push(c);
                    }
                }
                "tr" if depth == 1 => {
                    if !cur_row.is_empty() {
                        if let Some(tb) = cur.as_mut() {
                            tb.rows.push(std::mem::take(&mut cur_row));
                        }
                    }
                }
                "tbl" => {
                    if depth == 1 {
                        if let Some(mut tb) = cur.take() {
                            // 제목: 표의 첫 행 글자에서 찾는다 (한글은 제목도 표 안에 있다)
                            if tb.title.is_empty() {
                                if let Some(first) = tb.rows.first() {
                                    tb.title =
                                        first.iter().map(|c| c.text.clone()).collect::<String>();
                                }
                            }
                            tables.push(tb);
                        }
                    }
                    depth = depth.saturating_sub(1);
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(e) => panic!("XML 오류: {e}"),
            _ => {}
        }
    }
    tables
}

/// 설계안 6-1 1번: 표가 여럿이면 **제목 글자로 고른다**.
fn pick_quote_table(tables: &[Table]) -> (usize, &'static str) {
    let norm = |s: &str| -> String { s.chars().filter(|c| !c.is_whitespace()).collect() };
    for (i, t) in tables.iter().enumerate() {
        if norm(&t.title).contains("견적서") {
            return (i, "제목이 견적서");
        }
    }
    for (i, t) in tables.iter().enumerate() {
        let n = norm(&t.title);
        if !n.contains("납품서") && !n.contains("거래명세") {
            return (i, "납품서가 아닌 첫 표");
        }
    }
    (0, "첫 표(경고)")
}

fn grid(t: &Table) -> Vec<Vec<String>> {
    let width = t.rows.iter().flat_map(|r| r.iter().map(|c| c.col + c.col_span)).max().unwrap_or(0);
    let height = t.rows.iter().flat_map(|r| r.iter().map(|c| c.row + 1)).max().unwrap_or(0);
    let mut g = vec![vec![String::new(); width]; height];
    for row in &t.rows {
        for c in row {
            if c.row < height && c.col < width {
                g[c.row][c.col] = c.text.trim().to_string();
            }
        }
    }
    g
}

fn role_of(h: &str) -> Option<&'static str> {
    let h: String = h.chars().filter(|c| !c.is_whitespace()).collect();
    match h.as_str() {
        "품명" | "품목" | "물품명" | "상품명" | "내역" | "품명/규격" => Some("name"),
        "규격" | "사양" | "단위" => Some("spec"),
        "수량" => Some("qty"),
        "단가" => Some("unit_price"),
        "금액" | "공급가액" | "공급가" => Some("amount"),
        "세액" | "부가세" => Some("tax"),
        "비고" | "적요" => Some("note"),
        _ => None,
    }
}

/// 숫자 정규화. **괄호를 음수로 보는 규칙에 함정이 있다.**
/// 한글 견적서는 합계를 `(￦ 900,000 )` 처럼 괄호에 넣어 적는다 — 이건 음수가 아니다.
/// 그래서 괄호 안이 **숫자·쉼표·공백뿐일 때만** 회계식 음수로 본다.
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
    let neg = t.starts_with('-') || t.starts_with('△') || t.starts_with('▲') || paren_negative;
    let d: String = t.chars().filter(|c| c.is_ascii_digit()).collect();
    if d.is_empty() {
        return None;
    }
    let n: i64 = d.parse().ok()?;
    Some(if neg { -n } else { n })
}

const TOTAL_WORDS: [&str; 7] = ["합계", "총계", "소계", "계", "총액", "부가세", "VAT"];

#[test]
#[ignore = "P0-4 를 먼저 돌려 HWPX 를 만들어야 한다"]
fn p0_5_hwpx_quote_table() {
    let path = out_dir().join("p0-4-로봇과학.hwpx");
    assert!(path.exists(), "먼저 P0-4 를 돌려라: {}", path.display());

    let f = std::fs::File::open(&path).expect("HWPX 열기");
    let mut zip = zip::ZipArchive::new(f).expect("zip");
    let mut xml = String::new();
    zip.by_name("Contents/section0.xml").expect("section0").read_to_string(&mut xml).expect("읽기");

    let tables = parse_tables(&xml);
    println!("표 {}개", tables.len());
    for (i, t) in tables.iter().enumerate() {
        println!("  표{i}: 행 {} · 제목 {:?}", t.rows.len(), t.title.chars().take(30).collect::<String>());
    }
    assert_eq!(tables.len(), 2, "견적서와 납품서 두 표가 있어야 한다");

    let (pick, why) = pick_quote_table(&tables);
    println!("고른 표: {pick} ({why})");
    assert_eq!(pick, 0, "견적서 표를 골라야 한다");

    let g = grid(&tables[pick]);
    println!("격자 {}행 x {}열", g.len(), g.first().map(|r| r.len()).unwrap_or(0));

    // 머리글 행 찾기
    let hr = (0..g.len())
        .max_by_key(|r| g[*r].iter().filter(|c| role_of(c).is_some()).count())
        .expect("행");
    let hits = g[hr].iter().filter(|c| role_of(c).is_some()).count();
    println!("머리글 행 {hr} (일치 {hits}): {:?}", g[hr].iter().filter(|c| !c.is_empty()).collect::<Vec<_>>());
    assert!(hits >= 5, "머리글 낱말이 5개 이상 맞아야 한다");

    // 열 역할
    let mut roles: Vec<(usize, &'static str)> = Vec::new();
    for (c, h) in g[hr].iter().enumerate() {
        if let Some(role) = role_of(h) {
            if !roles.iter().any(|(_, r)| *r == role) {
                roles.push((c, role));
            }
        }
    }
    println!("열 역할: {roles:?}");
    let col = |role: &str| roles.iter().find(|(_, r)| *r == role).map(|(c, _)| *c);

    // 품목 뽑기
    let mut items: Vec<(String, String, Option<i64>, Option<i64>, Option<i64>)> = Vec::new();
    let mut totals: Vec<i64> = Vec::new();
    for r in (hr + 1)..g.len() {
        let get = |role: &str| col(role).and_then(|c| g[r].get(c)).cloned().unwrap_or_default();
        let name = get("name");
        let nm: String = name.chars().filter(|c| !c.is_whitespace()).collect();
        if TOTAL_WORDS.contains(&nm.as_str()) {
            // 합계 행 — 이 양식은 금액이 '규격' 자리에도 찍혀 있다. 공급가액 열 값을 쓴다.
            if let Some(a) = parse_number(&get("amount")) {
                totals.push(a);
            }
            continue;
        }
        if name.is_empty() {
            continue;
        }
        let amount = parse_number(&get("amount"));
        if amount.is_none() || amount == Some(0) {
            continue;
        }
        items.push((name, get("spec"), parse_number(&get("qty")), parse_number(&get("unit_price")), amount));
    }

    println!("품목 {}건, 합계 행 값 {:?}", items.len(), totals);
    for it in &items {
        println!("  {it:?}");
    }

    // --- 통과 기준 ---
    assert_eq!(items.len(), 2, "품목은 2건");
    assert_eq!(items[0].0, "프로보테크닉 교구");
    assert_eq!(items[0].1, "set");
    assert_eq!(items[0].2, Some(10));
    assert_eq!(items[0].3, Some(76_500));
    assert_eq!(items[0].4, Some(765_000));
    assert_eq!(items[1].0, "프로보테크닉 교재");
    assert_eq!(items[1].2, Some(10));
    assert_eq!(items[1].3, Some(13_500));
    assert_eq!(items[1].4, Some(135_000));

    // 수량 x 단가 = 금액
    for it in &items {
        assert_eq!(it.2.unwrap() * it.3.unwrap(), it.4.unwrap(), "{} 수량x단가", it.0);
    }
    let item_sum: i64 = items.iter().filter_map(|i| i.4).sum();
    assert_eq!(item_sum, 900_000, "품목 합");
    assert!(totals.contains(&900_000), "합계 행 값");

    // 합계금액 칸 (표 위쪽 '합 계 금 액(공급가액+세액)' 줄)
    let stated = g
        .iter()
        .flatten()
        .find(|c| c.contains('￦') || c.contains('₩'))
        .and_then(|c| parse_number(c));
    println!("합계금액 칸: {stated:?}");
    assert_eq!(stated, Some(900_000), "견적서에 적힌 합계금액");

    // 문구 생성
    let phrase = format!("로봇과학부 {} 외 {}종", items[0].0, items.len() - 1);
    assert_eq!(phrase, "로봇과학부 프로보테크닉 교구 외 1종");
    println!("자동 문구: {phrase}");

    // 납품서 표도 내용이 같은지 (그래서 제목으로 골라야 한다는 근거)
    let g2 = grid(&tables[1]);
    assert_eq!(g.len(), g2.len(), "두 표의 행 수가 같다");
    println!("납품서 표도 {}행 — 크기로는 구분 불가, 제목으로 골라야 한다", g2.len());
}
