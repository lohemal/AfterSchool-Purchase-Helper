//! P0 탐침 공용 유틸. 본 프로그램 코드가 아니다.

pub mod xlsx_spec;

use std::path::{Path, PathBuf};

/// 저장소 뿌리 (probe/ 의 부모)
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("probe 의 부모")
        .to_path_buf()
}

/// 실제 업무 샘플 (읽기 전용. 절대 고치지 않는다)
pub fn fixture(name: &str) -> PathBuf {
    repo_root().join("test").join("fixtures-local").join(name)
}

/// 시험 산출물 폴더
pub fn out_dir() -> PathBuf {
    let d = repo_root().join("test").join("out");
    std::fs::create_dir_all(&d).expect("test/out 만들기");
    d
}

pub fn out(name: &str) -> PathBuf {
    out_dir().join(name)
}

/// 샘플 원본이 손상되지 않았는지 확인하는 안전장치.
pub fn assert_fixture_readonly_size(name: &str, expected: u64) {
    let p = fixture(name);
    let len = std::fs::metadata(&p)
        .unwrap_or_else(|e| panic!("샘플을 찾을 수 없다 {}: {e}", p.display()))
        .len();
    assert_eq!(len, expected, "샘플 원본이 바뀌었다: {}", p.display());
}

/// COM 늦은 바인딩 헬퍼 — 문서 통합 도구에서 검증된 코드를 그대로 가져왔다.
pub mod com;
