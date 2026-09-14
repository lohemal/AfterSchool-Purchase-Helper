//! P4-5 — 프로그램이 쓰다 남긴 **임시 파일 치우기**.
//!
//! 한글 변환(`purchase-helper-hwp`)과 품의 파일 만들기(`purchase-helper-out`)는 `%TEMP%` 에 파일을 잠깐 만든다.
//! 정상적으로 끝나면 그 자리에서 지우지만, **프로그램이 갑자기 꺼지면 남는다.**
//! 그래서 시작할 때 한 번 훑어 **오래된 것만** 지운다.
//!
//! 지키는 것
//!   - 우리가 만든 폴더(`%TEMP%/purchase-helper-*`) 안만 본다. 사용자 자료는 건드리지 않는다.
//!   - 하루가 지난 것만 지운다. 지금 돌고 있는 다른 창의 작업 파일을 뺏지 않기 위해서다.
//!   - 지우다 실패해도 조용히 넘어간다. 임시 파일 때문에 프로그램이 안 켜지면 안 된다.

use std::path::Path;
use std::time::{Duration, SystemTime};

/// 이보다 오래된 임시 파일만 지운다
const KEEP: Duration = Duration::from_secs(24 * 60 * 60);

/// 우리가 만드는 임시 폴더들
const DIRS: &[&str] = &[
    "purchase-helper-hwp",
    "purchase-helper-out",
    // P5 에서 이름을 바꾸기 전 개발판이 남긴 것도 함께 치운다
    "quotemgr-hwp",
    "quotemgr-out",
];

/// 시작할 때 한 번 부른다. 지운 파일 수를 돌려준다.
pub fn sweep() -> usize {
    let base = std::env::temp_dir();
    let mut n = 0;
    for d in DIRS {
        n += sweep_dir(&base.join(d), KEEP);
    }
    n
}

/// 폴더 하나를 훑는다. 시험에서 기준 나이를 바꿔 부를 수 있게 나눠 두었다.
pub fn sweep_dir(dir: &Path, keep: Duration) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    let now = SystemTime::now();
    let mut n = 0;
    for e in entries.flatten() {
        let Ok(meta) = e.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let old = meta
            .modified()
            .ok()
            .and_then(|m| now.duration_since(m).ok())
            .map(|age| age >= keep)
            .unwrap_or(false);
        if old && std::fs::remove_file(e.path()).is_ok() {
            n += 1;
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("quotemgr-sweep-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn removes_only_old_files() {
        let d = dir("old");
        let fresh = d.join("방금.hwpx");
        std::fs::write(&fresh, b"x").unwrap();

        // 기준 나이를 0 으로 두면 전부 오래된 것이다
        assert_eq!(sweep_dir(&d, Duration::from_secs(0)), 1);
        assert!(!fresh.exists());

        // 기준이 길면 방금 만든 것은 남는다
        std::fs::write(&fresh, b"x").unwrap();
        assert_eq!(sweep_dir(&d, Duration::from_secs(3600)), 0);
        assert!(fresh.exists(), "지금 쓰고 있을 수도 있는 파일은 건드리지 않는다");

        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn leaves_folders_alone() {
        let d = dir("nested");
        let sub = d.join("안쪽");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("자료.xlsx"), b"x").unwrap();

        assert_eq!(sweep_dir(&d, Duration::from_secs(0)), 0, "폴더는 지우지 않는다");
        assert!(sub.join("자료.xlsx").exists());

        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn missing_folder_is_not_an_error() {
        assert_eq!(sweep_dir(Path::new("없는폴더-1234"), Duration::from_secs(0)), 0);
    }
}
