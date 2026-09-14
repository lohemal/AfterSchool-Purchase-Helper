//! 정산자료 파서 시험. 실제 샘플이 있으면 그걸로, 없으면 건너뛴다.

use super::*;

fn fixture(name: &str) -> Option<std::path::PathBuf> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()?
        .join("test")
        .join("fixtures-local")
        .join(name);
    p.exists().then_some(p)
}

const SAMPLE: &str = "품의_교재비·재료비_2026학년도_2026년9월_20260911.xlsx";

#[test]
fn real_settlement_file() {
    let Some(path) = fixture(SAMPLE) else {
        eprintln!("샘플 없음 — 건너뜀");
        return;
    };
    let f = read(&path).expect("정산자료 읽기");

    assert_eq!(f.sheet_name, "교재비·재료비 품의");
    assert_eq!(f.header_row, 1, "머리글은 1행");
    assert_eq!(f.rows.len(), 20, "자료 20행 (합계 행 제외)");

    // 합계 행이 자료에 섞이면 안 된다
    assert!(!f.rows.iter().any(|r| is_total_label(&r.source_name)));

    // 행마다 4재원 합 == 적힌 합계
    for r in &f.rows {
        assert_eq!(Some(r.funds.total()), r.stated_total, "{}", r.source_name);
    }

    // 열별 합 == 합계 행
    let tot = f.stated_totals.expect("합계 행");
    assert_eq!(tot.beneficiary, 7_862_900);
    assert_eq!(tot.excess, 50_300);
    assert_eq!(tot.subsidy, 1_923_500);
    assert_eq!(tot.voucher, 169_000);
    assert!(f.warnings.iter().all(|w| w.code != "SETTLE_COLUMN_SUM"), "{:?}", f.warnings);

    // 견적서 총액과 맞는 부서
    let find = |n: &str| f.rows.iter().find(|r| r.source_name == n).unwrap();
    assert_eq!(find("바둑").funds.total(), 444_000);
    assert_eq!(find("주산암산").funds.total(), 68_000);

    // 0원 재원
    assert_eq!(find("바둑").funds.voucher, 0);
    assert_eq!(find("우쿠렐레").funds.subsidy, 0);

    // 별칭이 여럿인 부서의 원문 이름이 그대로 남아 있다 (합산은 상위에서 한다)
    assert!(f.rows.iter().any(|r| r.source_name == "토탈공예미니어처1"));
    assert!(f.rows.iter().any(|r| r.source_name == "목요키즈쿠킹"));
}

/// 별칭 합산은 정산 파서가 아니라 부서 매핑 뒤에 일어난다 (설계안 8-2)
#[test]
fn alias_grouping_sums_funds() {
    let Some(path) = fixture(SAMPLE) else {
        eprintln!("샘플 없음 — 건너뜀");
        return;
    };
    let f = read(&path).unwrap();
    let pick = |names: &[&str]| {
        let mut sum = Funds::default();
        for n in names {
            sum.add(&f.rows.iter().find(|r| &r.source_name == n).unwrap().funds);
        }
        sum
    };
    let craft = pick(&["토탈공예미니어처", "토탈공예미니어처1", "토탈공예미니어처2"]);
    assert_eq!(craft.beneficiary, 690_000, "세 별칭의 수익자 합");
    let kids = pick(&["목요키즈쿠킹", "화요키즈쿠킹"]);
    assert_eq!(kids.beneficiary, 2_149_000);
}

#[test]
fn wrong_file_is_refused_not_guessed() {
    // 견적서를 정산자료로 넣으면 형식이 다르다고 멈춰야 한다
    let Some(path) = fixture("바둑 견적서.xlsx") else {
        eprintln!("샘플 없음 — 건너뜀");
        return;
    };
    let e = read(&path).unwrap_err();
    assert!(
        e.code.starts_with("SETTLE_"),
        "정산자료가 아니면 멈춰야 한다: {e}"
    );
}

#[test]
fn fund_column_synonyms() {
    assert_eq!(fund_of("수익자(1,2,4,5,6학년)"), Some(0));
    assert_eq!(fund_of("3학년 초과금"), Some(1));
    assert_eq!(fund_of("3학년 지원금"), Some(2));
    assert_eq!(fund_of("자유수강권"), Some(3));
    assert_eq!(fund_of("합계"), None);
    assert_eq!(fund_of("부서명"), None);
}

#[test]
fn name_header_synonyms() {
    assert!(is_name_header("부서명"));
    assert!(is_name_header("부서"));
    assert!(is_name_header("구분"));
    assert!(!is_name_header("수익자"));
}
