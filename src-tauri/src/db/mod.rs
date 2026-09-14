//! SQLite 연결 관리. 1인용 데스크톱 앱이라 `Mutex<Connection>` 하나면 충분하다.

pub mod migrate;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::Connection;

use crate::error::{AppError, AppResult};

pub struct Db {
    conn: Mutex<Connection>,
    path: PathBuf,
}

impl Db {
    pub fn open(path: &Path) -> AppResult<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut conn = Connection::open(path).map_err(|e| {
            AppError::new("DB_OPEN_FAILED", "자료 파일을 열지 못했습니다. 프로그램을 다시 시작해 주세요.")
                .detail(e.to_string())
        })?;
        setup_conn(&conn)?;
        migrate::run(&mut conn, path)?;
        Ok(Self { conn: Mutex::new(conn), path: path.to_path_buf() })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn read<T>(&self, f: impl FnOnce(&Connection) -> AppResult<T>) -> AppResult<T> {
        let guard = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        f(&guard)
    }

    /// 쓰기. 클로저가 Err 를 돌려주면 통째로 되돌린다.
    pub fn write<T>(&self, f: impl FnOnce(&Connection) -> AppResult<T>) -> AppResult<T> {
        let mut guard = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let tx = guard.transaction()?;
        let out = f(&tx)?;
        tx.commit()?;
        Ok(out)
    }

    pub fn schema_version(&self) -> AppResult<i32> {
        self.read(|c| Ok(migrate::current_version(c)?))
    }
}

fn setup_conn(conn: &Connection) -> AppResult<()> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(())
}

/// 시험용: 마이그레이션까지 끝낸 메모리 DB.
#[cfg(test)]
pub fn memory_db() -> Db {
    let mut conn = Connection::open_in_memory().expect("메모리 DB");
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    migrate::run(&mut conn, Path::new(":memory:")).expect("마이그레이션");
    Db { conn: Mutex::new(conn), path: PathBuf::from(":memory:") }
}

// ---------------------------------------------------------------- 공용 헬퍼

pub fn now() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

/// setting 표 읽기/쓰기
pub fn get_setting(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    let mut st = conn.prepare("SELECT value FROM setting WHERE key = ?1")?;
    let mut rows = st.query([key])?;
    Ok(match rows.next()? {
        Some(r) => Some(r.get(0)?),
        None => None,
    })
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO setting(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![key, value],
    )?;
    Ok(())
}

#[cfg(test)]
#[path = "crud_tests.rs"]
mod crud_tests;
