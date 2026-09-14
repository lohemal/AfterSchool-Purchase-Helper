//! P5 — 자료를 어디에 두는가, 그리고 **이름이 바뀌어도 잃지 않게** 하는 일.
//!
//! 자료는 프로그램이 설치된 자리가 아니라 **사용자 앱 데이터 폴더**에 둔다.
//! 그래야 프로그램을 지웠다 다시 깔아도, 새 버전으로 올려도 작업이 그대로 남는다.
//!
//! P5 에서 프로그램 이름을 `방과후 품의 도우미` 로 정하면서 자료 폴더 이름도 바뀌었다.
//! 개발판으로 실사용 시험을 하며 만들어 둔 작업이 사라진 것처럼 보이면 안 되므로,
//! **처음 한 번** 예전 자리에서 새 자리로 옮겨 온다.
//!
//! 지키는 것
//!   - **예전 자료를 지우지 않는다.** 복사만 한다. 잘못돼도 되돌릴 수 있어야 한다.
//!   - 새 자리에 이미 자료가 있으면 **건드리지 않는다.**
//!   - 옮기다 실패해도 프로그램은 그냥 새 자료로 시작한다. 켜지지 않으면 안 된다.
//!   - 파일을 그대로 복사하지 않고 **SQLite 가 스스로 복사**하게 한다(`backup`).
//!     그래야 아직 반영되지 않은 WAL 까지 온전히 따라온다.

use std::path::{Path, PathBuf};

use rusqlite::Connection;

/// 지금 쓰는 자료 파일 이름
pub const DB_FILE: &str = "purchase-helper.db";

/// 예전에 쓰던 (폴더 이름, 자료 파일 이름)
const LEGACY: &[(&str, &str)] = &[("kr.school.quotemgr", "quotemgr.db")];

/// 예전 이름으로 만들어 둔 자료 폴더들의 자리
pub fn legacy_dbs(app_data_root: &Path) -> Vec<PathBuf> {
    LEGACY.iter().map(|(dir, file)| app_data_root.join(dir).join(file)).collect()
}

/// 예전 자료를 새 자리로 **한 번** 옮겨 온다. 옮겼으면 어디서 가져왔는지 돌려준다.
///
/// 새 자리에 이미 자료가 있으면 아무것도 하지 않는다.
pub fn adopt_legacy(new_db: &Path, candidates: &[PathBuf]) -> Option<PathBuf> {
    if new_db.exists() {
        return None;
    }
    for old in candidates {
        if !old.exists() {
            continue;
        }
        match copy_database(old, new_db) {
            Ok(()) => return Some(old.clone()),
            Err(e) => {
                // 옮기지 못해도 프로그램은 켜져야 한다. 새 자료로 시작한다.
                log::warn!("예전 자료를 옮기지 못했습니다: {e}");
                let _ = std::fs::remove_file(new_db);
            }
        }
    }
    None
}

/// SQLite 에게 스스로 복사하게 한다 (아직 반영 안 된 WAL 까지 따라온다).
fn copy_database(from: &Path, to: &Path) -> rusqlite::Result<()> {
    if let Some(dir) = to.parent() {
        std::fs::create_dir_all(dir).map_err(|e| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CANTOPEN),
                Some(e.to_string()),
            )
        })?;
    }
    let src = Connection::open(from)?;
    let mut dst = Connection::open(to)?;
    {
        let backup = rusqlite::backup::Backup::new(&src, &mut dst)?;
        backup.run_to_completion(500, std::time::Duration::ZERO, None)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir()
            .join(format!("quotemgr-datadir-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// 예전 자료에 작업을 하나 만들어 둔다
    fn make_old(path: &Path, title: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut c = Connection::open(path).unwrap();
        crate::db::migrate::run(&mut c, path).unwrap();
        c.execute(
            "INSERT INTO work(title, school_year, month, kind, created_at, updated_at)
             VALUES (?1,'2026학년도','9월','교재비','2026-09-13','2026-09-13')",
            [title],
        )
        .unwrap();
    }

    #[test]
    fn brings_the_old_work_across() {
        let root = temp("adopt");
        let old = root.join("kr.school.quotemgr").join("quotemgr.db");
        let new = root.join("kr.school.afterschool-purchase-helper").join(DB_FILE);
        make_old(&old, "실사용 시험 작업");

        let from = adopt_legacy(&new, &legacy_dbs(&root)).expect("옮겨 와야 한다");
        assert_eq!(from, old);
        assert!(new.exists());

        // 작업이 그대로 있다
        let c = Connection::open(&new).unwrap();
        let title: String =
            c.query_row("SELECT title FROM work LIMIT 1", [], |r| r.get(0)).unwrap();
        assert_eq!(title, "실사용 시험 작업");

        // **예전 자료는 지우지 않는다**
        assert!(old.exists(), "예전 자료를 지우면 안 된다");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn never_overwrites_existing_data() {
        let root = temp("existing");
        let old = root.join("kr.school.quotemgr").join("quotemgr.db");
        let new = root.join("새자리").join(DB_FILE);
        make_old(&old, "예전 작업");
        make_old(&new, "지금 쓰는 작업");

        assert!(adopt_legacy(&new, &legacy_dbs(&root)).is_none(), "손대면 안 된다");

        let c = Connection::open(&new).unwrap();
        let title: String =
            c.query_row("SELECT title FROM work LIMIT 1", [], |r| r.get(0)).unwrap();
        assert_eq!(title, "지금 쓰는 작업", "쓰던 자료가 덮어써지면 안 된다");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn no_old_data_is_fine() {
        let root = temp("none");
        let new = root.join("새자리").join(DB_FILE);
        assert!(adopt_legacy(&new, &legacy_dbs(&root)).is_none());
        assert!(!new.exists(), "없는데 빈 파일을 만들면 안 된다");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// 예전 자료가 깨져 있어도 프로그램은 켜져야 한다
    #[test]
    fn broken_old_data_does_not_stop_the_app() {
        let root = temp("broken");
        let old = root.join("kr.school.quotemgr").join("quotemgr.db");
        std::fs::create_dir_all(old.parent().unwrap()).unwrap();
        std::fs::write(&old, b"this is not a database").unwrap();
        let new = root.join("새자리").join(DB_FILE);

        assert!(adopt_legacy(&new, &legacy_dbs(&root)).is_none());
        assert!(!new.exists(), "반쯤 만들어진 자료를 남기면 안 된다");

        // 그래도 새 자료로 시작할 수 있다
        let mut c = Connection::open(&new).unwrap();
        crate::db::migrate::run(&mut c, &new).unwrap();

        let _ = std::fs::remove_dir_all(&root);
    }
}

/// 설치본이 실제로 쓰는 자료를 들여다보는 진단 (사람이 확인할 때만 돌린다).
#[cfg(test)]
mod installed {
    use super::*;

    fn installed_db() -> Option<PathBuf> {
        let p = PathBuf::from(std::env::var("APPDATA").ok()?)
            .join("kr.school.afterschool-purchase-helper")
            .join(DB_FILE);
        p.exists().then_some(p)
    }

    /// P5 — **설치본이 쓰는 자료가 온전한가.**
    /// 설치해서 한 번 켠 뒤에 돌린다. 설치 자료가 없으면 건너뛴다.
    #[test]
    #[ignore = "설치본을 한 번 실행한 뒤에만 의미가 있다"]
    fn installed_app_data_is_intact() {
        let Some(path) = installed_db() else {
            eprintln!("건너뜀 — 설치본 자료가 없습니다. 설치하고 한 번 켠 뒤에 돌려 주세요.");
            return;
        };
        println!("설치본 자료: {}", path.display());

        let c = Connection::open(&path).expect("자료 열기");
        let version: i32 = c.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        println!("  스키마 v{version}");
        assert_eq!(version, crate::db::migrate::latest_version(), "스키마가 최신이어야 한다");

        let count = |sql: &str| -> i64 { c.query_row(sql, [], |r| r.get(0)).unwrap_or(-1) };
        let works = count("SELECT count(*) FROM work");
        let depts = count("SELECT count(*) FROM department");
        let quotes = count("SELECT count(*) FROM work_quote");
        let items = count("SELECT count(*) FROM work_quote_item");
        println!("  작업 {works} · 부서 {depts} · 견적서 {quotes} · 품목 {items}");

        let mut st = c.prepare("SELECT title, month, kind FROM work ORDER BY id").unwrap();
        let rows: Vec<(String, String, String)> = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .filter_map(|x| x.ok())
            .collect();
        for (t, m, k) in &rows {
            println!("  작업: {t} · {m} · {k}");
        }

        // 표가 다 있어야 한다 (옮겨 오다 반쯤 끊기지 않았는지)
        for t in ["department", "vendor_unit", "work", "work_quote", "work_quote_item"] {
            let n: i64 = c
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [t],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 1, "표가 없다: {t}");
        }
    }
}
