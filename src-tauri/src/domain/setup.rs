//! P1-2 부서 설정 — 품의 부서 · 정산 별칭 · 거래처 관리 단위.
//!
//! 셋은 **다른 개념**이다(설계안 2장). 이 파일에서 섞이지 않게 표를 따로 다룬다.
//!   - `settlement_alias` 는 정산자료를 부서로 모으는 데만 쓴다. **파일명 매칭에 쓰지 않는다.**
//!   - `vendor_unit.mgmt_name` 은 견적서 파일명에 넣는 이름이고 거래처를 가른다.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::db::now;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VendorUnit {
    #[serde(default)]
    pub id: i64,
    /// 거래처 관리명 — 견적서 파일명에 넣는 이름 ('로봇과학1')
    pub mgmt_name: String,
    /// 거래처 이름 (선택, 참고용)
    #[serde(default)]
    pub vendor_name: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub sort_order: i64,
    #[serde(default = "yes")]
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Department {
    #[serde(default)]
    pub id: i64,
    /// 품의 부서명 ('토탈공예미니어처')
    pub display_name: String,
    /// 품의 표기명 ('토탈공예미니어처부') — 모든 부서에 붙는다 (설계안 14장 11번)
    pub phrase_name: String,
    #[serde(default)]
    pub sort_order: i64,
    #[serde(default = "yes")]
    pub active: bool,
    /// 정산자료 '부서명' 열에 나오는 이름들. 거래처와 무관하다.
    #[serde(default)]
    pub aliases: Vec<String>,
    /// 거래처 관리 단위
    #[serde(default)]
    pub vendors: Vec<VendorUnit>,
}

fn yes() -> bool {
    true
}

/// 품의 표기명 기본값. 사람이 고칠 수 있다.
pub fn default_phrase_name(display_name: &str) -> String {
    format!("{display_name}부")
}

// ---------------------------------------------------------------- 읽기

pub fn list_departments(conn: &Connection) -> AppResult<Vec<Department>> {
    let mut st = conn.prepare(
        "SELECT id, display_name, phrase_name, sort_order, active
           FROM department ORDER BY sort_order, id",
    )?;
    let mut depts: Vec<Department> = st
        .query_map([], |r| {
            Ok(Department {
                id: r.get(0)?,
                display_name: r.get(1)?,
                phrase_name: r.get(2)?,
                sort_order: r.get(3)?,
                active: r.get::<_, i64>(4)? != 0,
                aliases: Vec::new(),
                vendors: Vec::new(),
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    for d in depts.iter_mut() {
        let mut a = conn
            .prepare("SELECT alias FROM settlement_alias WHERE department_id = ?1 ORDER BY id")?;
        d.aliases = a
            .query_map([d.id], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<_>>()?;

        let mut v = conn.prepare(
            "SELECT id, mgmt_name, COALESCE(vendor_name,''), COALESCE(note,''), sort_order, active
               FROM vendor_unit WHERE department_id = ?1 ORDER BY sort_order, id",
        )?;
        d.vendors = v
            .query_map([d.id], |r| {
                Ok(VendorUnit {
                    id: r.get(0)?,
                    mgmt_name: r.get(1)?,
                    vendor_name: r.get(2)?,
                    note: r.get(3)?,
                    sort_order: r.get(4)?,
                    active: r.get::<_, i64>(5)? != 0,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
    }
    Ok(depts)
}

/// 거래처 관리 단위 하나 → (부서명, 품의 표기명)
pub fn vendor_context(conn: &Connection, vendor_unit_id: i64) -> AppResult<(String, String, String)> {
    let mut st = conn.prepare(
        "SELECT v.mgmt_name, d.display_name, d.phrase_name
           FROM vendor_unit v JOIN department d ON d.id = v.department_id
          WHERE v.id = ?1",
    )?;
    let mut rows = st.query([vendor_unit_id])?;
    match rows.next()? {
        Some(r) => Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        None => Err(AppError::new("VENDOR_NOT_FOUND", "거래처를 찾을 수 없습니다.")),
    }
}

// ---------------------------------------------------------------- 쓰기

pub fn upsert_department(conn: &Connection, d: &Department) -> AppResult<i64> {
    let display = d.display_name.trim();
    if display.is_empty() {
        return Err(AppError::new("DEPT_NAME_EMPTY", "품의 부서명을 입력해 주세요."));
    }
    let phrase = if d.phrase_name.trim().is_empty() {
        default_phrase_name(display)
    } else {
        d.phrase_name.trim().to_string()
    };

    let id = if d.id > 0 {
        conn.execute(
            "UPDATE department SET display_name=?2, phrase_name=?3, sort_order=?4, active=?5
              WHERE id=?1",
            params![d.id, display, phrase, d.sort_order, d.active as i64],
        )
        .map_err(dup_dept)?;
        d.id
    } else {
        conn.execute(
            "INSERT INTO department(display_name, phrase_name, sort_order, active)
             VALUES (?1, ?2, ?3, ?4)",
            params![display, phrase, d.sort_order, d.active as i64],
        )
        .map_err(dup_dept)?;
        conn.last_insert_rowid()
    };

    // 별칭·거래처는 통째로 다시 심는다 (화면이 목록 전체를 보내 온다)
    conn.execute("DELETE FROM settlement_alias WHERE department_id = ?1", [id])?;
    for a in &d.aliases {
        let a = a.trim();
        if a.is_empty() {
            continue;
        }
        conn.execute(
            "INSERT INTO settlement_alias(department_id, alias) VALUES (?1, ?2)",
            params![id, a],
        )
        .map_err(|e| dup_alias(e, a))?;
    }

    // 거래처는 지우면 견적서 연결이 끊기므로 id 가 있는 것은 갱신한다
    let keep: Vec<i64> = d.vendors.iter().filter(|v| v.id > 0).map(|v| v.id).collect();
    let placeholders = if keep.is_empty() {
        "SELECT 0".to_string()
    } else {
        format!("SELECT {}", keep.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(" UNION SELECT "))
    };
    conn.execute(
        &format!("DELETE FROM vendor_unit WHERE department_id = ?1 AND id NOT IN ({placeholders})"),
        [id],
    )?;

    for v in &d.vendors {
        let name = v.mgmt_name.trim();
        if name.is_empty() {
            return Err(AppError::new("VENDOR_NAME_EMPTY", "거래처 관리명을 입력해 주세요."));
        }
        if v.id > 0 {
            conn.execute(
                "UPDATE vendor_unit SET mgmt_name=?2, vendor_name=?3, note=?4, sort_order=?5, active=?6
                  WHERE id=?1",
                params![v.id, name, v.vendor_name.trim(), v.note.trim(), v.sort_order, v.active as i64],
            )
            .map_err(|e| dup_vendor(e, name))?;
        } else {
            conn.execute(
                "INSERT INTO vendor_unit(department_id, mgmt_name, vendor_name, note, sort_order, active)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![id, name, v.vendor_name.trim(), v.note.trim(), v.sort_order, v.active as i64],
            )
            .map_err(|e| dup_vendor(e, name))?;
        }
    }
    Ok(id)
}

pub fn delete_department(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM department WHERE id = ?1", [id])?;
    Ok(())
}

/// 정산 별칭 하나를 부서에 붙인다 (정산자료 화면에서 미등록 이름을 만났을 때).
/// 붙인 내용은 설정에 남아 다음 달에 재사용된다 (설계안 8-2).
pub fn attach_alias(conn: &Connection, department_id: i64, alias: &str) -> AppResult<()> {
    let alias = alias.trim();
    if alias.is_empty() {
        return Err(AppError::new("ALIAS_EMPTY", "정산 별칭이 비어 있습니다."));
    }
    conn.execute(
        "INSERT INTO settlement_alias(department_id, alias) VALUES (?1, ?2)
         ON CONFLICT(alias) DO UPDATE SET department_id = excluded.department_id",
        params![department_id, alias],
    )?;
    Ok(())
}

/// 정산 별칭 → 품의 부서 id
pub fn resolve_alias(conn: &Connection, alias: &str) -> AppResult<Option<i64>> {
    let mut st = conn.prepare("SELECT department_id FROM settlement_alias WHERE alias = ?1")?;
    let mut rows = st.query([alias.trim()])?;
    Ok(match rows.next()? {
        Some(r) => Some(r.get(0)?),
        None => None,
    })
}

// ---------------------------------------------------------------- JSON 주고받기

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupExport {
    pub format: String,
    pub exported_at: String,
    pub departments: Vec<Department>,
}

pub fn export_setup(conn: &Connection) -> AppResult<SetupExport> {
    Ok(SetupExport {
        format: "quotemgr-setup-v1".into(),
        exported_at: now(),
        departments: list_departments(conn)?,
    })
}

/// 가져오기는 **통째로 바꾼다**. 작업 자료(work_*)는 건드리지 않는다.
pub fn import_setup(conn: &Connection, data: &SetupExport) -> AppResult<usize> {
    if data.format != "quotemgr-setup-v1" {
        return Err(AppError::new(
            "SETUP_FORMAT",
            "이 프로그램의 부서 설정 파일이 아닙니다.",
        )
        .detail(data.format.clone()));
    }
    conn.execute("DELETE FROM department", [])?;
    for (i, d) in data.departments.iter().enumerate() {
        let mut d = d.clone();
        d.id = 0;
        for v in d.vendors.iter_mut() {
            v.id = 0;
        }
        if d.sort_order == 0 {
            d.sort_order = i as i64;
        }
        upsert_department(conn, &d)?;
    }
    Ok(data.departments.len())
}

// ---------------------------------------------------------------- 오류 문장

fn dup_dept(e: rusqlite::Error) -> AppError {
    if is_unique(&e) {
        AppError::new("DEPT_DUPLICATE", "같은 이름의 품의 부서가 이미 있습니다.")
    } else {
        e.into()
    }
}

fn dup_alias(e: rusqlite::Error, alias: &str) -> AppError {
    if is_unique(&e) {
        AppError::new(
            "ALIAS_DUPLICATE",
            format!("정산 별칭 '{alias}' 는 이미 다른 부서에 등록되어 있습니다."),
        )
    } else {
        e.into()
    }
}

fn dup_vendor(e: rusqlite::Error, name: &str) -> AppError {
    if is_unique(&e) {
        AppError::new(
            "VENDOR_DUPLICATE",
            format!("거래처 관리명 '{name}' 은 이미 쓰이고 있습니다."),
        )
    } else {
        e.into()
    }
}

fn is_unique(e: &rusqlite::Error) -> bool {
    matches!(
        e,
        rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error { code: rusqlite::ErrorCode::ConstraintViolation, .. },
            _
        )
    )
}
