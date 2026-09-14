//! 에듀파인 생성기 시험. 실제 업로드에 쓴 샘플을 **골든 파일**로 삼아 셀 단위로 맞춘다.

use super::*;
use crate::domain::allocation::PumuiRow;

fn row(content: &str, amount: i64) -> PumuiRow {
    PumuiRow { vendor_unit_id: 0, content: content.into(), amount }
}

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join("quotemgr-test");
    std::fs::create_dir_all(&d).unwrap();
    d.join(name)
}

fn fixture(name: &str) -> Option<std::path::PathBuf> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()?
        .join("test")
        .join("fixtures-local")
        .join(name);
    p.exists().then_some(p)
}

#[test]
fn writes_and_verifies() {
    let path = tmp("생성-기본.xlsx");
    let rows = vec![
        row("바둑부 바둑교재(상상바둑) 1종", 372_000),
        row("주산암산부 방과후 기초Yap! 상 외 3종", 40_000),
    ];
    let r = create_verified(&path, &rows).expect("생성");
    assert_eq!(r.row_count, 2);
    assert_eq!(r.total, 412_000);
    assert!(path.exists());

    // 다시 읽어 구조 확인
    let book = read_book(&path);
    assert_eq!(book.sheet_names, vec!["품목내역".to_string()]);
    let s = &book.sheets[0];
    assert_eq!(s.dimension, "A1:D3");
    assert!(s.merged.is_empty());
    assert!(s.cells.iter().all(|c| c.formula.is_none()));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn refuses_empty_rows() {
    let path = tmp("생성-빈.xlsx");
    let e = create_verified(&path, &[]).unwrap_err();
    assert_eq!(e.code, "EDUFINE_NO_ROWS");
    assert!(!path.exists(), "0원 재원 파일을 만들면 안 된다");
}

/// 실제 업로드에 쓴 파일과 **셀 단위로 같아야 한다** (P0-1 의 기준을 본 프로그램에서 재확인)
#[test]
fn matches_real_edufine_sample() {
    let Some(sample_path) = fixture("9월 교재 재료 품의서(1).xlsx") else {
        eprintln!("샘플 없음 — 건너뜀");
        return;
    };
    let sample = read_book(&sample_path);
    let s = &sample.sheets[0];

    // 샘플에서 (내용, 예상단가)를 그대로 읽어 같은 행을 만든다
    let mut rows = Vec::new();
    let mut r = 2;
    loop {
        let Some(a) = s.cell(&format!("A{r}")) else { break };
        let Some(d) = s.cell(&format!("D{r}")) else { break };
        let content = match &a.value {
            CellValue::Text(t) => t.clone(),
            _ => break,
        };
        let amount: i64 = match &d.value {
            CellValue::Number(n) => n.parse().unwrap(),
            _ => break,
        };
        rows.push(row(&content, amount));
        r += 1;
    }
    assert_eq!(rows.len(), 19, "샘플은 19행");

    let path = tmp("생성-골든비교.xlsx");
    create_verified(&path, &rows).expect("생성");
    let made = read_book(&path);
    let m = &made.sheets[0];

    assert_eq!(made.sheet_names, sample.sheet_names);
    assert_eq!(m.dimension, s.dimension);
    assert_eq!(m.cells.len(), s.cells.len(), "칸 개수");

    let mut diffs = Vec::new();
    for (a, b) in s.cells.iter().zip(m.cells.iter()) {
        if a != b {
            diffs.push(format!("{}\n  샘플: {}\n  생성: {}", a.reference, a.describe(), b.describe()));
        }
    }
    assert!(diffs.is_empty(), "샘플과 다른 곳 {}군데:\n{}", diffs.len(), diffs.join("\n"));

    let _ = std::fs::remove_file(&path);
}

#[test]
fn verify_catches_wrong_sheet_name() {
    // 시트 이름이 다른 파일을 만들어 검증이 잡아내는지 본다
    let path = tmp("생성-틀린시트.xlsx");
    {
        let mut wb = rust_xlsxwriter::Workbook::new();
        let ws = wb.add_worksheet();
        ws.set_name("엉뚱한이름").unwrap();
        ws.write_string(0, 0, "내용").unwrap();
        wb.save(&path).unwrap();
    }
    let e = verify_file(&path, &[row("x", 1)]).unwrap_err();
    assert_eq!(e.code, "EDUFINE_VERIFY");
    assert!(e.message.contains("품목내역"), "{}", e.message);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn verify_catches_wrong_amount() {
    let path = tmp("생성-값불일치.xlsx");
    let rows = vec![row("바둑부 바둑교재 1종", 372_000)];
    write_file(&path, &rows).unwrap();
    // 다른 금액을 기대하면 실패해야 한다
    let e = verify_file(&path, &[row("바둑부 바둑교재 1종", 999)]).unwrap_err();
    assert!(e.message.contains("예상단가"), "{}", e.message);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn file_names() {
    assert_eq!(
        file_name("2026학년도", "9월", "교재비", "수익자"),
        "2026학년도_9월_교재비_수익자.xlsx"
    );
    // 빈 칸은 건너뛴다
    assert_eq!(file_name("", "9월", "", "지원금"), "9월_지원금.xlsx");
}

#[test]
fn spec_constants_are_fixed() {
    // 에듀파인이 요구하는 값들이 바뀌지 않게 못 박아 둔다
    assert_eq!(SHEET_NAME, "품목내역");
    assert_eq!(HEADERS, ["내용", "규격", "수량", "예상단가"]);
    assert_eq!(SPEC_TEXT, "식");
    assert_eq!(QTY, 1.0);
}

/// P4-5 — **이미 있는 파일을 덮어쓰지 않는다.**
/// 먼저 만든 품의 파일이 이미 결재에 올라가 있을 수 있다.
#[test]
fn never_overwrites_an_existing_file() {
    let dir = std::env::temp_dir().join(format!("quotemgr-overwrite-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let rows = vec![row("교재A", 30_000)];
    let dest = dir.join("2026학년도_9월_교재비_수익자부담.xlsx");

    let first = create_verified(&dest, &rows).unwrap();
    assert_eq!(first.path, dest, "처음에는 원하는 이름 그대로");

    let second = create_verified(&dest, &rows).unwrap();
    assert_ne!(second.path, dest, "두 번째는 옆 이름으로 비켜 간다");
    assert_eq!(
        second.path.file_name().unwrap().to_string_lossy(),
        "2026학년도_9월_교재비_수익자부담 (2).xlsx"
    );
    assert!(dest.exists(), "먼저 만든 파일이 그대로 있어야 한다");

    let third = create_verified(&dest, &rows).unwrap();
    assert_eq!(
        third.path.file_name().unwrap().to_string_lossy(),
        "2026학년도_9월_교재비_수익자부담 (3).xlsx"
    );

    // 셋 다 제대로 만들어졌다
    for p in [&first.path, &second.path, &third.path] {
        assert!(p.exists(), "{p:?}");
        verify_file(p, &rows).unwrap();
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn free_path_keeps_the_name_when_nothing_is_there() {
    let p = std::env::temp_dir().join(format!("quotemgr-없는파일-{}.xlsx", std::process::id()));
    let _ = std::fs::remove_file(&p);
    assert_eq!(free_path(&p), p);
}
