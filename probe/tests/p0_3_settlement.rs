//! P0-3  정산자료 파싱
//!
//! 시험 내용: 정산자료 20행을 뽑고, 합계 행을 제외하며, 열별 합이 합계 행과 맞는지 검산한다.
//! 각 행의 `4재원 합 == 합계` 도 확인한다. 머리글 행 번호를 고정하지 않는다.
//! 샘플: test/fixtures-local/품의_교재비·재료비_2026학년도_2026년9월_20260911.xlsx (읽기만)

use calamine::{open_workbook, Data, Reader, Xlsx};
use probe::fixture;

const SAMPLE: &str = "품의_교재비·재료비_2026학년도_2026년9월_20260911.xlsx";

/// 재원 네 가지. 열 이름은 학교마다 다를 수 있어 낱말로 찾는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fund {
    Beneficiary, // 수익자
    Excess,      // 초과금
    Subsidy,     // 지원금
    Voucher,     // 자유수강권
}

fn fund_of(header: &str) -> Option<Fund> {
    let h: String = header.chars().filter(|c| !c.is_whitespace()).collect();
    if h.contains("수익자") {
        Some(Fund::Beneficiary)
    } else if h.contains("초과금") {
        Some(Fund::Excess)
    } else if h.contains("지원금") {
        Some(Fund::Subsidy)
    } else if h.contains("자유수강권") {
        Some(Fund::Voucher)
    } else {
        None
    }
}

fn is_name_header(header: &str) -> bool {
    let h: String = header.chars().filter(|c| !c.is_whitespace()).collect();
    h == "부서명" || h == "부서" || h == "강좌명" || h == "구분"
}

fn is_total_header(header: &str) -> bool {
    let h: String = header.chars().filter(|c| !c.is_whitespace()).collect();
    h == "합계" || h == "계" || h == "총계"
}

fn text(d: Option<&Data>) -> String {
    match d {
        Some(Data::String(s)) => s.trim().to_string(),
        Some(Data::Float(f)) => format!("{}", *f as i64),
        Some(Data::Int(i)) => i.to_string(),
        _ => String::new(),
    }
}

fn number(d: Option<&Data>) -> Option<i64> {
    match d {
        Some(Data::Float(f)) => Some(f.round() as i64),
        Some(Data::Int(i)) => Some(*i),
        Some(Data::String(s)) => {
            let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
            if digits.is_empty() {
                None
            } else {
                let n: i64 = digits.parse().ok()?;
                Some(if s.trim_start().starts_with('-') { -n } else { n })
            }
        }
        _ => None,
    }
}

#[derive(Debug)]
struct Row {
    source_name: String,
    beneficiary: i64,
    excess: i64,
    subsidy: i64,
    voucher: i64,
    stated_total: Option<i64>,
}

impl Row {
    fn sum4(&self) -> i64 {
        self.beneficiary + self.excess + self.subsidy + self.voucher
    }
}

#[test]
fn p0_3_settlement_parse() {
    let path = fixture(SAMPLE);
    let mut wb: Xlsx<_> = open_workbook(&path).expect("정산자료 열기");
    println!("시트: {:?}", wb.sheet_names());
    assert_eq!(wb.sheet_names().len(), 1, "시트는 하나");
    let sheet = wb.sheet_names()[0].clone();
    assert_eq!(sheet, "교재비·재료비 품의");

    let range = wb.worksheet_range(&sheet).expect("시트 읽기");
    let (row0, _col0) = range.start().expect("사용 영역");
    let (h, w) = (range.height(), range.width());
    println!("크기 {h}행 x {w}열, 시작 엑셀 {}행", row0 + 1);

    // --- 머리글 찾기 (위에서 10행 안에서 낱말로. 행 번호를 고정하지 않는다) ---
    let mut header_row = None;
    for r in 0..h.min(10) {
        let has_name = (0..w).any(|c| is_name_header(&text(range.get((r, c)))));
        let funds = (0..w).filter(|c| fund_of(&text(range.get((r, *c)))).is_some()).count();
        if has_name && funds >= 3 {
            header_row = Some(r);
            break;
        }
    }
    let hr = header_row.expect("머리글 행을 찾지 못했다");
    println!("머리글 행: 엑셀 {}행", row0 as usize + hr + 1);

    // --- 열 역할 ---
    let name_col = (0..w).find(|c| is_name_header(&text(range.get((hr, *c))))).expect("부서명 열");
    let mut fund_cols: Vec<(usize, Fund)> = Vec::new();
    for c in 0..w {
        if let Some(f) = fund_of(&text(range.get((hr, c)))) {
            fund_cols.push((c, f));
        }
    }
    let total_col = (0..w).find(|c| is_total_header(&text(range.get((hr, *c)))));
    println!("부서명 열 {name_col}, 재원 열 {fund_cols:?}, 합계 열 {total_col:?}");

    // 설계안 8-3: 필요한 열을 못 찾으면 추정하지 말고 멈춘다
    for want in [Fund::Beneficiary, Fund::Excess, Fund::Subsidy, Fund::Voucher] {
        assert!(fund_cols.iter().any(|(_, f)| *f == want), "재원 열을 못 찾았다: {want:?}");
    }
    assert_eq!(fund_cols.len(), 4, "재원 열은 정확히 4개");

    // --- 자료 행 / 합계 행 나누기 ---
    let get_fund = |r: usize, want: Fund| -> i64 {
        fund_cols
            .iter()
            .find(|(_, f)| *f == want)
            .and_then(|(c, _)| number(range.get((r, *c))))
            .unwrap_or(0)
    };

    let mut rows = Vec::new();
    let mut total_row: Option<Row> = None;
    for r in (hr + 1)..h {
        let name = text(range.get((hr.max(r), name_col)));
        if name.is_empty() {
            continue;
        }
        let row = Row {
            source_name: name.clone(),
            beneficiary: get_fund(r, Fund::Beneficiary),
            excess: get_fund(r, Fund::Excess),
            subsidy: get_fund(r, Fund::Subsidy),
            voucher: get_fund(r, Fund::Voucher),
            stated_total: total_col.and_then(|c| number(range.get((r, c)))),
        };
        // A열이 '합계' 인 행은 자료에서 제외하고 검산에 쓴다 (설계안 8-2)
        if is_total_header(&name) {
            total_row = Some(row);
        } else {
            rows.push(row);
        }
    }

    println!("자료 행 {}개, 합계 행 {}", rows.len(), total_row.is_some());
    for row in &rows {
        println!(
            "  {:<16} 수익자 {:>9} 초과 {:>6} 지원 {:>8} 자유 {:>7} 합계 {:?}",
            row.source_name, row.beneficiary, row.excess, row.subsidy, row.voucher, row.stated_total
        );
    }

    // --- 통과 기준 ---
    assert_eq!(rows.len(), 20, "자료 행은 20개");
    let tr = total_row.expect("합계 행이 있어야 한다");

    // 1) 각 행에서 4재원 합 == 표기 합계
    for row in &rows {
        assert_eq!(
            Some(row.sum4()),
            row.stated_total,
            "{} 의 4재원 합이 합계와 다르다",
            row.source_name
        );
    }

    // 2) 열별 합 == 합계 행 (설계안 8-2 형식 검산)
    let sum = |f: fn(&Row) -> i64| rows.iter().map(f).sum::<i64>();
    assert_eq!(sum(|r| r.beneficiary), tr.beneficiary, "수익자 열 합");
    assert_eq!(sum(|r| r.excess), tr.excess, "초과금 열 합");
    assert_eq!(sum(|r| r.subsidy), tr.subsidy, "지원금 열 합");
    assert_eq!(sum(|r| r.voucher), tr.voucher, "자유수강권 열 합");
    assert_eq!(Some(sum(|r| r.sum4())), tr.stated_total, "총합");

    println!(
        "열별 합: 수익자 {} · 초과금 {} · 지원금 {} · 자유수강권 {} · 총합 {:?}",
        tr.beneficiary, tr.excess, tr.subsidy, tr.voucher, tr.stated_total
    );
    assert_eq!(tr.beneficiary, 7_862_900);
    assert_eq!(tr.excess, 50_300);
    assert_eq!(tr.subsidy, 1_923_500);
    assert_eq!(tr.voucher, 169_000);
    assert_eq!(tr.stated_total, Some(10_005_700));

    // 3) 숫자 셀인지 (문자로 들어오면 안 된다)
    let first_data = hr + 1;
    for (c, _) in &fund_cols {
        let v = range.get((first_data, *c));
        assert!(
            matches!(v, Some(Data::Float(_)) | Some(Data::Int(_))),
            "재원 금액은 숫자 셀이어야 한다: {v:?}"
        );
    }

    // 4) 별칭 합산 (설계안 8-2) — 여러 정산 행이 한 품의 부서로 합쳐진다
    let find = |n: &str| rows.iter().find(|r| r.source_name == n).unwrap();
    let total_craft: i64 = ["토탈공예미니어처", "토탈공예미니어처1", "토탈공예미니어처2"]
        .iter()
        .map(|n| find(n).beneficiary)
        .sum();
    assert_eq!(total_craft, 690_000, "토탈공예미니어처 세 별칭의 수익자 합");
    let kids: i64 = ["목요키즈쿠킹", "화요키즈쿠킹"].iter().map(|n| find(n).beneficiary).sum();
    assert_eq!(kids, 2_149_000, "키즈쿠킹 두 별칭의 수익자 합");
    println!("별칭 합산 확인: 토탈공예미니어처 {total_craft}, 키즈쿠킹 {kids}");

    // 5) 견적서 총액과 맞는지 (거래처가 하나인 부서)
    assert_eq!(find("바둑").stated_total, Some(444_000), "바둑 = 바둑 견적서 총액");
    assert_eq!(find("주산암산").stated_total, Some(68_000), "주산암산 = 주산암산 견적서 총액");

    // 6) 0원 재원은 품의 행을 만들지 않는다 (설계안 14장 14번)
    assert_eq!(find("바둑").voucher, 0, "바둑 자유수강권은 0");
    assert_eq!(find("우쿠렐레").subsidy, 0, "우쿠렐레 지원금은 0");
}
