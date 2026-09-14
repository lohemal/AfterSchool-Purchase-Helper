//! 마이그레이션 러너.
//!
//! `PRAGMA user_version` 을 스키마 번호로 쓴다.
//! 새 마이그레이션은 `migrations/00N_설명.sql` 을 만들고 아래 목록에 한 줄 더한다.
//! **이미 적용된 파일은 절대 고치지 않는다** (쓰던 사람의 DB 가 어긋난다).

use std::path::Path;

use rusqlite::Connection;

use crate::error::{AppError, AppResult};

struct Migration {
    version: i32,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "001_init",
        sql: include_str!("../../migrations/001_init.sql"),
    },
    Migration {
        version: 2,
        name: "002_quote_source",
        sql: include_str!("../../migrations/002_quote_source.sql"),
    },
];

pub fn latest_version() -> i32 {
    MIGRATIONS.iter().map(|m| m.version).max().unwrap_or(0)
}

pub fn current_version(conn: &Connection) -> rusqlite::Result<i32> {
    conn.query_row("PRAGMA user_version", [], |r| r.get(0))
}

pub fn run(conn: &mut Connection, db_path: &Path) -> AppResult<()> {
    let from = current_version(conn)?;
    let to = latest_version();

    if from == to {
        return Ok(());
    }
    if from > to {
        return Err(AppError::new(
            "SCHEMA_TOO_NEW",
            "더 최신 버전의 프로그램에서 만든 자료입니다. 프로그램을 최신 버전으로 올려 주세요.",
        )
        .detail(format!("db={from} app={to}")));
    }

    // 쓰던 자료가 있으면 먼저 복사해 둔다
    if from > 0 && db_path != Path::new(":memory:") && db_path.exists() {
        let dir = db_path.parent().unwrap_or_else(|| Path::new(".")).join("backups");
        std::fs::create_dir_all(&dir)?;
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let dest = dir.join(format!("before-v{to}-{stamp}.db"));
        let mut out = Connection::open(&dest)?;
        {
            let b = rusqlite::backup::Backup::new(conn, &mut out)?;
            b.run_to_completion(500, std::time::Duration::ZERO, None)?;
        }
        let _: String =
            out.pragma_update_and_check(None, "journal_mode", "DELETE", |r| r.get(0))?;
    }

    for m in MIGRATIONS.iter().filter(|m| m.version > from) {
        let tx = conn.transaction()?;
        tx.execute_batch(m.sql).map_err(|e| {
            AppError::new("MIGRATION_FAILED", "자료 구조를 바꾸는 중 문제가 생겼습니다.")
                .detail(format!("{} :: {e}", m.name))
        })?;
        tx.pragma_update(None, "user_version", m.version)?;
        tx.commit()?;
        log::info!("마이그레이션 적용: {}", m.name);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_db_reaches_latest() {
        let mut c = Connection::open_in_memory().unwrap();
        run(&mut c, Path::new(":memory:")).unwrap();
        assert_eq!(current_version(&c).unwrap(), latest_version());
    }

    #[test]
    fn running_twice_is_safe() {
        let mut c = Connection::open_in_memory().unwrap();
        run(&mut c, Path::new(":memory:")).unwrap();
        run(&mut c, Path::new(":memory:")).unwrap();
        assert_eq!(current_version(&c).unwrap(), latest_version());
    }

    #[test]
    fn refuses_newer_schema() {
        let mut c = Connection::open_in_memory().unwrap();
        c.pragma_update(None, "user_version", latest_version() + 1).unwrap();
        let e = run(&mut c, Path::new(":memory:")).unwrap_err();
        assert_eq!(e.code, "SCHEMA_TOO_NEW");
    }

    /// P1 로 쓰던 자료(v1)를 열어도 **그대로 열리고** 새 칸만 붙는다.
    /// P1 에서 읽어 둔 견적서는 모두 파일에서 그대로 읽은 것이므로 `structured` 다.
    #[test]
    fn v1_database_upgrades_without_losing_anything() {
        let mut c = Connection::open_in_memory().unwrap();
        // v1 까지만 적용한 상태를 만든다
        c.execute_batch(MIGRATIONS[0].sql).unwrap();
        c.pragma_update(None, "user_version", 1).unwrap();
        c.execute(
            "INSERT INTO work(title, school_year, month, kind, created_at, updated_at)
             VALUES ('시험','2026','9월','교재비','2026-09-11','2026-09-11')",
            [],
        )
        .unwrap();
        c.execute(
            "INSERT INTO work_quote(work_id, source_path, source_name, format, parse_status, grand_total)
             VALUES (1, 'C:/x/바둑 견적서.xlsx', '바둑 견적서.xlsx', 'xlsx', 'ok', 444000)",
            [],
        )
        .unwrap();

        run(&mut c, Path::new(":memory:")).unwrap();
        assert_eq!(current_version(&c).unwrap(), latest_version());

        let (name, total, source, trust, raw): (String, i64, String, String, String) = c
            .query_row(
                "SELECT source_name, grand_total, source, trust, raw_grand_total FROM work_quote WHERE id=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .unwrap();
        assert_eq!(name, "바둑 견적서.xlsx", "쓰던 자료가 그대로 있어야 한다");
        assert_eq!(total, 444_000);
        assert_eq!(source, "structured", "P1 자료는 파일에서 그대로 읽은 것이다");
        assert_eq!(trust, "");
        assert_eq!(raw, "");
    }

    #[test]
    fn all_tables_exist() {
        let mut c = Connection::open_in_memory().unwrap();
        run(&mut c, Path::new(":memory:")).unwrap();
        let want = [
            "department",
            "settlement_alias",
            "vendor_unit",
            "setting",
            "work",
            "work_quote",
            "work_quote_item",
            "work_settlement_row",
            "work_allocation",
            "work_check",
            "work_output",
        ];
        for t in want {
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
