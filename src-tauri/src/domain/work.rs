//! P1-11 작업 저장과 이어하기 + 각 단계의 자료를 DB 에 담고 꺼내는 자리.
//!
//! 원본 견적서 파일은 **복사하지 않는다**. 경로·크기·해시만 두고,
//! 파일이 옮겨지거나 지워져도 **이미 뽑아 둔 자료로 작업을 계속할 수 있다** (설계안 14장 15번).

use std::collections::HashMap;
use std::path::Path;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::db::now;
use crate::domain::allocation::{
    self, Check, CheckStatus, DeptAllocation, PumuiRow, VendorAllocation,
};
use crate::domain::matching::{MatchCandidate, MatchMethod};
use crate::domain::phrase::{self, PhraseRow};
use crate::domain::status::{Blocker, GateResult};
use crate::domain::{CompareBasis, Confidence, Fund, Funds, RowKind, SignEffect, Warning, ALL_FUNDS};
use crate::error::{AppError, AppResult};
use crate::quote::model::{ParsedQuote, QuoteFormat};

// ---------------------------------------------------------------- 작업

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Work {
    pub id: i64,
    pub title: String,
    pub school_year: String,
    pub month: String,
    pub kind: String,
    pub created_at: String,
    pub updated_at: String,
    pub status: String,
}

pub fn create_work(
    conn: &Connection,
    title: &str,
    school_year: &str,
    month: &str,
    kind: &str,
) -> AppResult<i64> {
    let t = title.trim();
    if t.is_empty() {
        return Err(AppError::new("WORK_TITLE_EMPTY", "작업 이름을 입력해 주세요."));
    }
    let ts = now();
    conn.execute(
        "INSERT INTO work(title, school_year, month, kind, created_at, updated_at, status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5, 'open')",
        params![t, school_year.trim(), month.trim(), kind.trim(), ts],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn list_works(conn: &Connection) -> AppResult<Vec<Work>> {
    let mut st = conn.prepare(
        "SELECT id, title, school_year, month, kind, created_at, updated_at, status
           FROM work ORDER BY updated_at DESC, id DESC",
    )?;
    let out = st
        .query_map([], |r| {
            Ok(Work {
                id: r.get(0)?,
                title: r.get(1)?,
                school_year: r.get(2)?,
                month: r.get(3)?,
                kind: r.get(4)?,
                created_at: r.get(5)?,
                updated_at: r.get(6)?,
                status: r.get(7)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(out)
}

pub fn get_work(conn: &Connection, id: i64) -> AppResult<Work> {
    list_works(conn)?
        .into_iter()
        .find(|w| w.id == id)
        .ok_or_else(|| AppError::new("WORK_NOT_FOUND", "작업을 찾을 수 없습니다."))
}

pub fn touch_work(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("UPDATE work SET updated_at = ?2 WHERE id = ?1", params![id, now()])?;
    Ok(())
}

pub fn delete_work(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM work WHERE id = ?1", [id])?;
    Ok(())
}

// ---------------------------------------------------------------- 견적서 등록

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteRow {
    pub id: i64,
    pub vendor_unit_id: Option<i64>,
    pub vendor_mgmt_name: String,
    pub department_name: String,
    pub phrase_name: String,
    pub source_path: String,
    pub source_name: String,
    pub source_exists: bool,
    pub format: String,
    pub match_method: String,
    pub match_note: String,
    pub parse_status: String,
    pub parse_error: String,
    pub table_note: String,
    pub supply_total: Option<i64>,
    pub tax_total: Option<i64>,
    pub grand_total: Option<i64>,
    pub item_sum: i64,
    pub adjustment_sum: i64,
    pub computed_total: i64,
    pub compare_basis: String,
    pub compare_total: i64,
    /// 어느 경로로 읽었는가 (structured | pdf_text | ocr)
    pub source: String,
    /// 한 줄 신뢰 상태 (ok | needs_check | uncertain | amount_failed)
    pub trust: String,
    /// 사진에서 읽은 합계 원문 — **고치지 않고 보여 주기만 한다**
    pub raw_grand_total: String,
    pub raw_supply_total: String,
    pub representative_item_id: Option<i64>,
    pub content_phrase: String,
    pub content_phrase_auto: String,
    pub content_phrase_override: Option<String>,
    pub vendor_name_in_doc: String,
    pub warnings: Vec<Warning>,
    pub items: Vec<ItemRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemRow {
    pub id: i64,
    pub row_no: i64,
    pub kind: RowKind,
    pub sign_effect: Option<SignEffect>,
    pub display_name: String,
    pub spec: String,
    pub qty: Option<i64>,
    pub unit_price: Option<i64>,
    pub amount: Option<i64>,
    pub raw_name: String,
    pub raw_spec: String,
    pub raw_qty: String,
    pub raw_unit_price: String,
    pub raw_amount: String,
    pub edited: bool,
    pub cell_ref: String,
    pub confidence: Confidence,
    pub warnings: Vec<Warning>,
}

fn file_hash(path: &Path) -> String {
    let Ok(bytes) = std::fs::read(path) else { return String::new() };
    let mut h = Sha256::new();
    h.update(&bytes);
    format!("{:x}", h.finalize())
}

/// 매칭 후보 — **거래처 관리명과 품의 부서명만**. 정산 별칭은 넣지 않는다.
pub fn match_candidates(conn: &Connection) -> AppResult<Vec<MatchCandidate>> {
    let mut st = conn.prepare(
        "SELECT v.id, v.mgmt_name, d.id, d.display_name
           FROM vendor_unit v JOIN department d ON d.id = v.department_id
          WHERE v.active = 1 AND d.active = 1
          ORDER BY d.sort_order, v.sort_order",
    )?;
    let out = st
        .query_map([], |r| {
            Ok(MatchCandidate {
                vendor_unit_id: r.get(0)?,
                mgmt_name: r.get(1)?,
                department_id: r.get(2)?,
                department_name: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(out)
}

/// 견적서 파일 하나를 등록하고 (가능하면) 바로 읽는다.
pub fn register_quote(conn: &Connection, work_id: i64, path: &Path) -> AppResult<i64> {
    let source_name = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let format = QuoteFormat::from_path(path);
    let size = std::fs::metadata(path).map(|m| m.len() as i64).unwrap_or(0);
    let hash = file_hash(path);

    // 1) 파일명 매칭
    let cands = match_candidates(conn)?;
    let m = crate::domain::matching::match_file(&source_name, &cands);

    // 같은 거래처에 이미 견적서가 있으면 막는다 (설계안 8-1)
    if let Some(vid) = m.vendor_unit_id {
        let dup: i64 = conn.query_row(
            "SELECT count(*) FROM work_quote WHERE work_id = ?1 AND vendor_unit_id = ?2",
            params![work_id, vid],
            |r| r.get(0),
        )?;
        if dup > 0 {
            return Err(AppError::new(
                "QUOTE_DUPLICATE_VENDOR",
                "이 거래처에는 이미 견적서가 등록돼 있습니다. 먼저 지우고 다시 넣어 주세요.",
            ));
        }
    }

    conn.execute(
        "INSERT INTO work_quote(work_id, vendor_unit_id, source_path, source_name, source_size,
                                source_hash, format, match_method, match_note, parse_status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'pending')",
        params![
            work_id,
            m.vendor_unit_id,
            path.to_string_lossy(),
            source_name,
            size,
            hash,
            format.key(),
            m.method.key(),
            m.note,
        ],
    )?;
    let quote_id = conn.last_insert_rowid();

    // 2) 읽기 — 못 읽는 형식이어도 **등록은 남긴다**(작업이 멈추면 안 된다)
    match crate::quote::parse(path) {
        Ok(parsed) => store_parsed(conn, quote_id, &parsed)?,
        Err(e) => {
            conn.execute(
                "UPDATE work_quote SET parse_status='failed', parse_error=?2 WHERE id=?1",
                params![quote_id, e.message],
            )?;
        }
    }
    refresh_phrase(conn, quote_id)?;
    touch_work(conn, work_id)?;
    Ok(quote_id)
}

fn store_parsed(conn: &Connection, quote_id: i64, q: &ParsedQuote) -> AppResult<()> {
    conn.execute("DELETE FROM work_quote_item WHERE work_quote_id = ?1", [quote_id])?;
    for (i, it) in q.items.iter().enumerate() {
        conn.execute(
            "INSERT INTO work_quote_item(work_quote_id, row_no, kind, sign_effect, display_name,
                 spec, qty, unit_price, amount, raw_name, raw_spec, raw_qty, raw_unit_price,
                 raw_amount, edited, cell_ref, confidence, warnings)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,0,?15,?16,?17)",
            params![
                quote_id,
                i as i64,
                it.kind.key(),
                it.sign_effect.map(|s| s.key()),
                it.display_name,
                it.spec,
                it.qty,
                it.unit_price,
                it.amount,
                it.raw_name,
                it.raw_spec,
                it.raw_qty,
                it.raw_unit_price,
                it.raw_amount,
                it.cell_ref,
                it.confidence.key(),
                serde_json::to_string(&it.warnings)?,
            ],
        )?;
    }
    conn.execute(
        "UPDATE work_quote SET parse_status='ok', parse_error='', parsed_at=?2, table_note=?3,
             supply_total=?4, tax_total=?5, grand_total=?6, compare_basis=?7,
             vendor_name_in_doc=?8, warnings=?9, source=?10, trust=?11,
             raw_grand_total=?12, raw_supply_total=?13
           WHERE id=?1",
        params![
            quote_id,
            now(),
            q.table_note,
            q.supply_total,
            q.tax_total,
            q.grand_total,
            q.compare_basis.key(),
            q.vendor_name_in_doc,
            serde_json::to_string(&q.warnings)?,
            q.source,
            q.trust,
            q.raw_grand_total,
            q.raw_supply_total,
        ],
    )?;
    recompute_quote_totals(conn, quote_id)
}

/// 품목이 바뀔 때마다 합계를 다시 센다.
pub fn recompute_quote_totals(conn: &Connection, quote_id: i64) -> AppResult<()> {
    let items = load_items(conn, quote_id)?;
    let item_sum: i64 = items
        .iter()
        .filter(|i| i.kind == RowKind::Item)
        .filter_map(|i| i.amount)
        .sum();
    let adjustment_sum: i64 = items
        .iter()
        .filter(|i| i.kind == RowKind::Adjustment)
        .filter_map(|i| {
            i.amount.map(|a| i.sign_effect.unwrap_or(SignEffect::Unknown).apply(a))
        })
        .sum();
    conn.execute(
        "UPDATE work_quote SET item_sum=?2, adjustment_sum=?3, computed_total=?4 WHERE id=?1",
        params![quote_id, item_sum, adjustment_sum, item_sum + adjustment_sum],
    )?;
    Ok(())
}

pub fn load_items(conn: &Connection, quote_id: i64) -> AppResult<Vec<ItemRow>> {
    let mut st = conn.prepare(
        "SELECT id, row_no, kind, sign_effect, display_name, spec, qty, unit_price, amount,
                raw_name, raw_spec, raw_qty, raw_unit_price, raw_amount, edited, cell_ref,
                confidence, warnings
           FROM work_quote_item WHERE work_quote_id = ?1 ORDER BY row_no, id",
    )?;
    let out = st
        .query_map([quote_id], |r| {
            let warnings: String = r.get(17)?;
            let conf: String = r.get(16)?;
            let kind: String = r.get(2)?;
            let sign: Option<String> = r.get(3)?;
            Ok(ItemRow {
                id: r.get(0)?,
                row_no: r.get(1)?,
                kind: RowKind::from_key(&kind).unwrap_or(RowKind::Item),
                sign_effect: sign.as_deref().and_then(SignEffect::from_key),
                display_name: r.get(4)?,
                spec: r.get(5)?,
                qty: r.get(6)?,
                unit_price: r.get(7)?,
                amount: r.get(8)?,
                raw_name: r.get(9)?,
                raw_spec: r.get(10)?,
                raw_qty: r.get(11)?,
                raw_unit_price: r.get(12)?,
                raw_amount: r.get(13)?,
                edited: r.get::<_, i64>(14)? != 0,
                cell_ref: r.get(15)?,
                confidence: match conf.as_str() {
                    "low" => Confidence::Low,
                    "medium" => Confidence::Medium,
                    _ => Confidence::High,
                },
                warnings: serde_json::from_str(&warnings).unwrap_or_default(),
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(out)
}

/// 품목 한 줄을 고친다. **`raw_*` 는 절대 덮어쓰지 않는다.**
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemEdit {
    pub id: i64,
    pub kind: RowKind,
    pub sign_effect: Option<SignEffect>,
    pub display_name: String,
    pub spec: String,
    pub qty: Option<i64>,
    pub unit_price: Option<i64>,
    pub amount: Option<i64>,
}

pub fn update_item(conn: &Connection, edit: &ItemEdit) -> AppResult<i64> {
    let quote_id: i64 = conn.query_row(
        "SELECT work_quote_id FROM work_quote_item WHERE id = ?1",
        [edit.id],
        |r| r.get(0),
    )?;
    conn.execute(
        "UPDATE work_quote_item
            SET kind=?2, sign_effect=?3, display_name=?4, spec=?5, qty=?6, unit_price=?7,
                amount=?8, edited=1
          WHERE id=?1",
        params![
            edit.id,
            edit.kind.key(),
            edit.sign_effect.map(|s| s.key()),
            edit.display_name.trim(),
            edit.spec.trim(),
            edit.qty,
            edit.unit_price,
            edit.amount,
        ],
    )?;
    recompute_quote_totals(conn, quote_id)?;
    refresh_phrase(conn, quote_id)?;
    Ok(quote_id)
}

pub fn add_item(conn: &Connection, quote_id: i64) -> AppResult<i64> {
    let next: i64 = conn.query_row(
        "SELECT COALESCE(MAX(row_no), -1) + 1 FROM work_quote_item WHERE work_quote_id = ?1",
        [quote_id],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO work_quote_item(work_quote_id, row_no, kind, display_name, edited, confidence)
         VALUES (?1, ?2, 'item', '', 1, 'high')",
        params![quote_id, next],
    )?;
    let id = conn.last_insert_rowid();
    refresh_phrase(conn, quote_id)?;
    Ok(id)
}

pub fn delete_item(conn: &Connection, item_id: i64) -> AppResult<i64> {
    let quote_id: i64 = conn.query_row(
        "SELECT work_quote_id FROM work_quote_item WHERE id = ?1",
        [item_id],
        |r| r.get(0),
    )?;
    conn.execute("DELETE FROM work_quote_item WHERE id = ?1", [item_id])?;
    recompute_quote_totals(conn, quote_id)?;
    refresh_phrase(conn, quote_id)?;
    Ok(quote_id)
}

/// 자동 문구를 다시 만든다. 사용자가 고친 문구(`override`)는 건드리지 않는다.
pub fn refresh_phrase(conn: &Connection, quote_id: i64) -> AppResult<()> {
    let phrase_name: Option<String> = conn
        .query_row(
            "SELECT d.phrase_name FROM work_quote q
               JOIN vendor_unit v ON v.id = q.vendor_unit_id
               JOIN department d ON d.id = v.department_id
              WHERE q.id = ?1",
            [quote_id],
            |r| r.get(0),
        )
        .ok();

    let items = load_items(conn, quote_id)?;
    let rep: Option<i64> = conn
        .query_row("SELECT representative_item_id FROM work_quote WHERE id = ?1", [quote_id], |r| {
            r.get(0)
        })
        .ok()
        .flatten();

    let rows: Vec<PhraseRow> = items
        .iter()
        .map(|i| PhraseRow { kind: i.kind, display_name: i.display_name.clone() })
        .collect();
    let rep_idx = rep.and_then(|id| items.iter().position(|i| i.id == id));

    let auto = match &phrase_name {
        Some(p) => phrase::build(p, &rows, rep_idx).unwrap_or_default(),
        None => String::new(),
    };
    conn.execute(
        "UPDATE work_quote SET content_phrase_auto = ?2 WHERE id = ?1",
        params![quote_id, auto],
    )?;
    Ok(())
}

pub fn set_representative(conn: &Connection, quote_id: i64, item_id: Option<i64>) -> AppResult<()> {
    conn.execute(
        "UPDATE work_quote SET representative_item_id = ?2 WHERE id = ?1",
        params![quote_id, item_id],
    )?;
    refresh_phrase(conn, quote_id)
}

pub fn set_phrase_override(conn: &Connection, quote_id: i64, text: Option<&str>) -> AppResult<()> {
    let v = text.map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
    conn.execute(
        "UPDATE work_quote SET content_phrase_override = ?2 WHERE id = ?1",
        params![quote_id, v],
    )?;
    Ok(())
}

pub fn set_compare_basis(conn: &Connection, quote_id: i64, basis: CompareBasis) -> AppResult<()> {
    conn.execute(
        "UPDATE work_quote SET compare_basis = ?2 WHERE id = ?1",
        params![quote_id, basis.key()],
    )?;
    Ok(())
}

/// 사용자가 거래처를 직접 고른다.
pub fn set_quote_vendor(conn: &Connection, quote_id: i64, vendor_unit_id: Option<i64>) -> AppResult<()> {
    if let Some(vid) = vendor_unit_id {
        let work_id: i64 =
            conn.query_row("SELECT work_id FROM work_quote WHERE id = ?1", [quote_id], |r| r.get(0))?;
        let dup: i64 = conn.query_row(
            "SELECT count(*) FROM work_quote WHERE work_id = ?1 AND vendor_unit_id = ?2 AND id <> ?3",
            params![work_id, vid, quote_id],
            |r| r.get(0),
        )?;
        if dup > 0 {
            return Err(AppError::new(
                "QUOTE_DUPLICATE_VENDOR",
                "이 거래처에는 이미 다른 견적서가 붙어 있습니다.",
            ));
        }
    }
    conn.execute(
        "UPDATE work_quote SET vendor_unit_id = ?2, match_method = ?3, match_note = '' WHERE id = ?1",
        params![
            quote_id,
            vendor_unit_id,
            if vendor_unit_id.is_some() { MatchMethod::Manual.key() } else { MatchMethod::None.key() }
        ],
    )?;
    refresh_phrase(conn, quote_id)
}

pub fn delete_quote(conn: &Connection, quote_id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM work_quote WHERE id = ?1", [quote_id])?;
    Ok(())
}

/// 견적서를 다시 읽는다 (원본 파일이 있어야 한다).
pub fn reparse_quote(conn: &Connection, quote_id: i64) -> AppResult<()> {
    let path: String =
        conn.query_row("SELECT source_path FROM work_quote WHERE id = ?1", [quote_id], |r| r.get(0))?;
    let p = Path::new(&path);
    if !p.exists() {
        return Err(AppError::new(
            "QUOTE_SOURCE_MISSING",
            "원본 견적서 파일을 찾을 수 없습니다. 이미 읽어 둔 내용은 그대로 쓸 수 있습니다.",
        ));
    }
    match crate::quote::parse(p) {
        Ok(parsed) => store_parsed(conn, quote_id, &parsed)?,
        Err(e) => {
            conn.execute(
                "UPDATE work_quote SET parse_status='failed', parse_error=?2 WHERE id=?1",
                params![quote_id, e.message],
            )?;
            return Err(e);
        }
    }
    refresh_phrase(conn, quote_id)
}

pub fn load_quotes(conn: &Connection, work_id: i64) -> AppResult<Vec<QuoteRow>> {
    let mut st = conn.prepare(
        "SELECT q.id, q.vendor_unit_id, COALESCE(v.mgmt_name,''), COALESCE(d.display_name,''),
                COALESCE(d.phrase_name,''), q.source_path, q.source_name, q.format,
                q.match_method, q.match_note, q.parse_status, q.parse_error, q.table_note,
                q.supply_total, q.tax_total, q.grand_total, q.item_sum, q.adjustment_sum,
                q.computed_total, q.compare_basis, q.representative_item_id,
                q.content_phrase_auto, q.content_phrase_override, q.vendor_name_in_doc, q.warnings,
                q.source, q.trust, q.raw_grand_total, q.raw_supply_total
           FROM work_quote q
           LEFT JOIN vendor_unit v ON v.id = q.vendor_unit_id
           LEFT JOIN department d ON d.id = v.department_id
          WHERE q.work_id = ?1
          ORDER BY d.sort_order, v.sort_order, q.id",
    )?;

    let mut out: Vec<QuoteRow> = st
        .query_map([work_id], |r| {
            let warnings: String = r.get(24)?;
            let basis: String = r.get(19)?;
            let path: String = r.get(5)?;
            let auto: String = r.get(21)?;
            let over: Option<String> = r.get(22)?;
            Ok(QuoteRow {
                id: r.get(0)?,
                vendor_unit_id: r.get(1)?,
                vendor_mgmt_name: r.get(2)?,
                department_name: r.get(3)?,
                phrase_name: r.get(4)?,
                source_exists: Path::new(&path).exists(),
                source_path: path,
                source_name: r.get(6)?,
                format: r.get(7)?,
                match_method: r.get(8)?,
                match_note: r.get(9)?,
                parse_status: r.get(10)?,
                parse_error: r.get(11)?,
                table_note: r.get(12)?,
                supply_total: r.get(13)?,
                tax_total: r.get(14)?,
                grand_total: r.get(15)?,
                item_sum: r.get(16)?,
                adjustment_sum: r.get(17)?,
                computed_total: r.get(18)?,
                compare_total: 0,
                compare_basis: basis,
                source: r.get(25)?,
                trust: r.get(26)?,
                raw_grand_total: r.get(27)?,
                raw_supply_total: r.get(28)?,
                representative_item_id: r.get(20)?,
                content_phrase: over.clone().unwrap_or_else(|| auto.clone()),
                content_phrase_auto: auto,
                content_phrase_override: over,
                vendor_name_in_doc: r.get(23)?,
                warnings: serde_json::from_str(&warnings).unwrap_or_default(),
                items: Vec::new(),
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    for q in out.iter_mut() {
        q.items = load_items(conn, q.id)?;
        q.compare_total = compare_total_of(q);
    }
    Ok(out)
}

/// 정산과 비교할 값
pub fn compare_total_of(q: &QuoteRow) -> i64 {
    match CompareBasis::from_key(&q.compare_basis).unwrap_or(CompareBasis::Grand) {
        CompareBasis::Grand => q.grand_total.unwrap_or(q.computed_total),
        CompareBasis::Supply => q.supply_total.unwrap_or(q.computed_total),
        CompareBasis::ComputedTotal => q.computed_total,
    }
}

// ---------------------------------------------------------------- 정산자료

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettlementRowView {
    pub id: i64,
    pub row_no: i64,
    pub source_name: String,
    pub department_id: Option<i64>,
    pub department_name: String,
    pub funds: Funds,
    pub stated_total: Option<i64>,
}

/// 정산자료를 읽어 담는다. 별칭 → 부서 연결은 `settlement_alias` 표에서만 찾는다.
pub fn load_settlement(conn: &Connection, work_id: i64, path: &Path) -> AppResult<Vec<Warning>> {
    let file = crate::settlement::read(path)?;
    conn.execute("DELETE FROM work_settlement_row WHERE work_id = ?1", [work_id])?;
    for row in &file.rows {
        let dept = crate::domain::setup::resolve_alias(conn, &row.source_name)?;
        conn.execute(
            "INSERT INTO work_settlement_row(work_id, row_no, source_name, department_id,
                 beneficiary, excess, subsidy, voucher, stated_total)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                work_id,
                row.row_no,
                row.source_name,
                dept,
                row.funds.beneficiary,
                row.funds.excess,
                row.funds.subsidy,
                row.funds.voucher,
                row.stated_total,
            ],
        )?;
    }
    touch_work(conn, work_id)?;
    Ok(file.warnings)
}

pub fn settlement_rows(conn: &Connection, work_id: i64) -> AppResult<Vec<SettlementRowView>> {
    let mut st = conn.prepare(
        "SELECT s.id, s.row_no, s.source_name, s.department_id, COALESCE(d.display_name,''),
                s.beneficiary, s.excess, s.subsidy, s.voucher, s.stated_total
           FROM work_settlement_row s
           LEFT JOIN department d ON d.id = s.department_id
          WHERE s.work_id = ?1 ORDER BY s.row_no",
    )?;
    let out = st
        .query_map([work_id], |r| {
            Ok(SettlementRowView {
                id: r.get(0)?,
                row_no: r.get(1)?,
                source_name: r.get(2)?,
                department_id: r.get(3)?,
                department_name: r.get(4)?,
                funds: Funds {
                    beneficiary: r.get(5)?,
                    excess: r.get(6)?,
                    subsidy: r.get(7)?,
                    voucher: r.get(8)?,
                },
                stated_total: r.get(9)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(out)
}

/// 미등록 별칭을 부서에 붙이고, 이번 작업의 줄에도 반영한다.
pub fn map_alias(conn: &Connection, work_id: i64, source_name: &str, department_id: i64) -> AppResult<()> {
    crate::domain::setup::attach_alias(conn, department_id, source_name)?;
    conn.execute(
        "UPDATE work_settlement_row SET department_id = ?3 WHERE work_id = ?1 AND source_name = ?2",
        params![work_id, source_name, department_id],
    )?;
    touch_work(conn, work_id)
}

// ---------------------------------------------------------------- 배분·검증

/// 부서별로 정산 금액을 모으고 거래처 배분을 붙인다.
pub fn dept_allocations(conn: &Connection, work_id: i64) -> AppResult<Vec<DeptAllocation>> {
    // 정산 금액을 부서로 합산 (별칭 여럿 → 한 부서)
    let mut settle: HashMap<i64, Funds> = HashMap::new();
    for row in settlement_rows(conn, work_id)? {
        if let Some(d) = row.department_id {
            settle.entry(d).or_default().add(&row.funds);
        }
    }

    // 견적서 총액
    let quotes = load_quotes(conn, work_id)?;
    let mut quote_total: HashMap<i64, i64> = HashMap::new();
    for q in &quotes {
        if let Some(v) = q.vendor_unit_id {
            quote_total.insert(v, q.compare_total);
        }
    }

    // 저장된 배분
    let mut alloc: HashMap<i64, (Funds, String)> = HashMap::new();
    {
        let mut st = conn.prepare(
            "SELECT vendor_unit_id, beneficiary, excess, subsidy, voucher, source
               FROM work_allocation WHERE work_id = ?1",
        )?;
        let rows = st.query_map([work_id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                Funds {
                    beneficiary: r.get(1)?,
                    excess: r.get(2)?,
                    subsidy: r.get(3)?,
                    voucher: r.get(4)?,
                },
                r.get::<_, String>(5)?,
            ))
        })?;
        for row in rows {
            let (v, f, s) = row?;
            alloc.insert(v, (f, s));
        }
    }

    // 부서 + 거래처 목록
    let mut st = conn.prepare(
        "SELECT d.id, d.display_name, v.id, v.mgmt_name
           FROM department d LEFT JOIN vendor_unit v ON v.department_id = d.id AND v.active = 1
          WHERE d.active = 1
          ORDER BY d.sort_order, d.id, v.sort_order, v.id",
    )?;
    let rows: Vec<(i64, String, Option<i64>, Option<String>)> = st
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<rusqlite::Result<_>>()?;

    let mut out: Vec<DeptAllocation> = Vec::new();
    for (did, dname, vid, vname) in rows {
        if out.last().map(|d| d.department_id) != Some(did) {
            out.push(DeptAllocation {
                department_id: did,
                department_name: dname.clone(),
                settlement: settle.get(&did).copied().unwrap_or_default(),
                has_settlement: settle.contains_key(&did),
                vendors: Vec::new(),
            });
        }
        if let (Some(vid), Some(vname)) = (vid, vname) {
            let saved = alloc.get(&vid);
            out.last_mut().unwrap().vendors.push(VendorAllocation {
                vendor_unit_id: vid,
                mgmt_name: vname,
                allocated: saved.map(|(f, _)| *f).unwrap_or_default(),
                missing: saved.is_none(),
                quote_total: quote_total.get(&vid).copied(),
            });
        }
    }

    // 거래처가 하나인 부서는 정산 금액을 **그대로 옮긴다** (계산이 아니다)
    for d in out.iter_mut() {
        if let Some((vid, funds)) = allocation::auto_allocation(d) {
            if let Some(v) = d.vendors.iter_mut().find(|v| v.vendor_unit_id == vid) {
                if v.missing || alloc.get(&vid).map(|(_, s)| s == "auto").unwrap_or(false) {
                    v.allocated = funds;
                    v.missing = false;
                }
            }
        }
    }
    Ok(out)
}

/// 거래처가 하나인 부서의 배분을 DB 에 써 둔다 (화면이 열릴 때 한 번).
pub fn apply_auto_allocations(conn: &Connection, work_id: i64) -> AppResult<usize> {
    let depts = dept_allocations(conn, work_id)?;
    let mut n = 0;
    for d in &depts {
        if let Some((vid, funds)) = allocation::auto_allocation(d) {
            save_allocation(conn, work_id, vid, funds, true)?;
            n += 1;
        }
    }
    Ok(n)
}

pub fn save_allocation(
    conn: &Connection,
    work_id: i64,
    vendor_unit_id: i64,
    funds: Funds,
    auto: bool,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO work_allocation(work_id, vendor_unit_id, beneficiary, excess, subsidy,
                                     voucher, source, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)
         ON CONFLICT(work_id, vendor_unit_id) DO UPDATE SET
             beneficiary=excluded.beneficiary, excess=excluded.excess,
             subsidy=excluded.subsidy, voucher=excluded.voucher,
             source=excluded.source, updated_at=excluded.updated_at",
        params![
            work_id,
            vendor_unit_id,
            funds.beneficiary,
            funds.excess,
            funds.subsidy,
            funds.voucher,
            if auto { "auto" } else { "manual" },
            now(),
        ],
    )?;
    touch_work(conn, work_id)
}

/// 모든 검증을 다시 돌려 `work_check` 에 담는다. **확인 사유는 유지한다.**
pub fn run_checks(conn: &Connection, work_id: i64) -> AppResult<Vec<Check>> {
    // 이미 확인한 항목의 사유를 기억해 둔다 (키: kind + 부서 + 거래처 + 재원)
    let mut acked: HashMap<String, (i64, String, String)> = HashMap::new();
    {
        let mut st = conn.prepare(
            "SELECT kind, COALESCE(department_id,0), COALESCE(vendor_unit_id,0),
                    COALESCE(fund,''), acknowledged, ack_reason, COALESCE(ack_at,'')
               FROM work_check WHERE work_id = ?1 AND acknowledged = 1",
        )?;
        let rows = st.query_map([work_id], |r| {
            Ok((
                format!(
                    "{}|{}|{}|{}",
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, String>(3)?
                ),
                r.get::<_, i64>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
            ))
        })?;
        for row in rows {
            let (k, a, reason, at) = row?;
            acked.insert(k, (a, reason, at));
        }
    }

    let depts = dept_allocations(conn, work_id)?;
    let mut all: Vec<Check> = Vec::new();
    for d in &depts {
        // 견적서도 정산도 없는 부서는 이번 달 대상이 아니다 — 조용히 넘어간다
        if !d.has_settlement && d.vendors.iter().all(|v| v.quote_total.is_none()) {
            continue;
        }
        all.extend(allocation::verify_department(d));
    }

    // 미등록 별칭
    for row in settlement_rows(conn, work_id)? {
        if row.department_id.is_none() {
            all.push(Check {
                kind: "unmapped_alias".into(),
                department_id: None,
                vendor_unit_id: None,
                fund: None,
                label: format!(
                    "정산자료의 '{}' 이 어느 품의 부서에도 연결되지 않았습니다.",
                    row.source_name
                ),
                expected: Some(row.funds.total()),
                actual: None,
                diff: None,
                status: CheckStatus::Error,
            });
        }
    }

    // 거래처 매칭 실패
    for q in load_quotes(conn, work_id)? {
        if q.vendor_unit_id.is_none() {
            all.push(Check {
                kind: "unmatched_quote".into(),
                department_id: None,
                vendor_unit_id: None,
                fund: None,
                label: format!("견적서 '{}' 의 거래처를 정하지 않았습니다.", q.source_name),
                expected: None,
                actual: None,
                diff: None,
                status: CheckStatus::Error,
            });
        }
    }

    conn.execute("DELETE FROM work_check WHERE work_id = ?1", [work_id])?;
    for c in &all {
        let key = format!(
            "{}|{}|{}|{}",
            c.kind,
            c.department_id.unwrap_or(0),
            c.vendor_unit_id.unwrap_or(0),
            c.fund.clone().unwrap_or_default()
        );
        let (ack, reason, at) = acked
            .get(&key)
            .cloned()
            .unwrap_or((0, String::new(), String::new()));
        conn.execute(
            "INSERT INTO work_check(work_id, kind, department_id, vendor_unit_id, fund, label,
                 expected, actual, diff, status, acknowledged, ack_reason, ack_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
            params![
                work_id,
                c.kind,
                c.department_id,
                c.vendor_unit_id,
                c.fund,
                c.label,
                c.expected,
                c.actual,
                c.diff,
                c.status.key(),
                ack,
                reason,
                if at.is_empty() { None } else { Some(at) },
            ],
        )?;
    }
    touch_work(conn, work_id)?;
    Ok(all)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckRow {
    pub id: i64,
    pub kind: String,
    pub label: String,
    pub status: String,
    pub expected: Option<i64>,
    pub actual: Option<i64>,
    pub diff: Option<i64>,
    pub acknowledged: bool,
    pub ack_reason: String,
}

pub fn list_checks(conn: &Connection, work_id: i64) -> AppResult<Vec<CheckRow>> {
    let mut st = conn.prepare(
        "SELECT id, kind, label, status, expected, actual, diff, acknowledged, ack_reason
           FROM work_check WHERE work_id = ?1
          ORDER BY CASE status WHEN 'error' THEN 0 WHEN 'warn' THEN 1 ELSE 2 END, id",
    )?;
    let out = st
        .query_map([work_id], |r| {
            Ok(CheckRow {
                id: r.get(0)?,
                kind: r.get(1)?,
                label: r.get(2)?,
                status: r.get(3)?,
                expected: r.get(4)?,
                actual: r.get(5)?,
                diff: r.get(6)?,
                acknowledged: r.get::<_, i64>(7)? != 0,
                ack_reason: r.get(8)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(out)
}

/// 불일치를 확인하고 사유를 적는다. **공란은 받지 않는다.**
pub fn acknowledge(conn: &Connection, check_id: i64, reason: &str) -> AppResult<()> {
    crate::domain::status::validate_reason(reason)
        .map_err(|m| AppError::new("ACK_REASON_EMPTY", m))?;
    conn.execute(
        "UPDATE work_check SET acknowledged = 1, ack_reason = ?2, ack_at = ?3 WHERE id = ?1",
        params![check_id, reason.trim(), now()],
    )?;
    Ok(())
}

pub fn unacknowledge(conn: &Connection, check_id: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE work_check SET acknowledged = 0, ack_reason = '', ack_at = NULL WHERE id = ?1",
        [check_id],
    )?;
    Ok(())
}

/// 생성해도 되는지
pub fn gate(conn: &Connection, work_id: i64) -> AppResult<GateResult> {
    let blockers: Vec<Blocker> = list_checks(conn, work_id)?
        .into_iter()
        .filter(|c| c.status == "error")
        .map(|c| Blocker {
            check_id: c.id,
            kind: c.kind,
            label: c.label,
            acknowledged: c.acknowledged,
            ack_reason: c.ack_reason,
        })
        .collect();
    Ok(crate::domain::status::gate(blockers))
}

// ---------------------------------------------------------------- 미리보기·생성

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FundPreview {
    pub fund: String,
    pub fund_label: String,
    pub rows: Vec<PumuiRow>,
    pub total: i64,
    /// 행이 없을 때 화면에 띄울 문장
    pub empty_note: String,
}

/// 재원별 최종 행. **금액이 0인 행은 만들지 않는다.**
pub fn preview(conn: &Connection, work_id: i64) -> AppResult<Vec<FundPreview>> {
    let quotes = load_quotes(conn, work_id)?;
    let depts = dept_allocations(conn, work_id)?;

    // 거래처 → 문구
    let mut phrase_of: HashMap<i64, String> = HashMap::new();
    for q in &quotes {
        if let Some(v) = q.vendor_unit_id {
            if !q.content_phrase.trim().is_empty() {
                phrase_of.insert(v, q.content_phrase.clone());
            }
        }
    }

    let mut entries: Vec<(i64, String, Funds)> = Vec::new();
    for d in &depts {
        for v in &d.vendors {
            if let Some(p) = phrase_of.get(&v.vendor_unit_id) {
                entries.push((v.vendor_unit_id, p.clone(), v.allocated));
            }
        }
    }

    Ok(ALL_FUNDS
        .into_iter()
        .map(|f| {
            let rows = allocation::rows_for_fund(f, &entries);
            let total = rows.iter().map(|r| r.amount).sum();
            let empty_note = if rows.is_empty() {
                format!("{}: 대상 금액 없음 · 파일 생성 안 함", f.label())
            } else {
                String::new()
            };
            FundPreview {
                fund: f.key().to_string(),
                fund_label: f.label().to_string(),
                rows,
                total,
                empty_note,
            }
        })
        .collect())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateResult {
    pub created: Vec<CreatedFile>,
    pub skipped: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedFile {
    pub fund_label: String,
    pub path: String,
    pub row_count: usize,
    pub total: i64,
}

/// 파일을 만든다. 빨강이 남아 있으면 **막는다** (설계안 14장 13번).
pub fn generate(conn: &Connection, work_id: i64, dest_dir: &Path) -> AppResult<GenerateResult> {
    let g = gate(conn, work_id)?;
    if !g.can_generate {
        return Err(AppError::new(
            "GENERATE_BLOCKED",
            format!("{} 확인하거나 사유를 적어야 파일을 만들 수 있습니다.", g.message),
        ));
    }
    let w = get_work(conn, work_id)?;
    let previews = preview(conn, work_id)?;

    conn.execute("DELETE FROM work_output WHERE work_id = ?1", [work_id])?;
    let mut created = Vec::new();
    let mut skipped = Vec::new();

    for p in &previews {
        let fund = Fund::from_key(&p.fund).unwrap_or(Fund::Beneficiary);
        if p.rows.is_empty() {
            skipped.push(p.empty_note.clone());
            continue;
        }
        let name = crate::edufine::file_name(&w.school_year, &w.month, &w.kind, fund.label());
        let wanted = dest_dir.join(&name);
        let report = crate::edufine::create_verified(&wanted, &p.rows)?;
        // 같은 이름이 이미 있으면 덮어쓰지 않고 옆 이름으로 만든다 — 실제 자리를 기록한다
        let path = report.path.clone();

        conn.execute(
            "INSERT INTO work_output(work_id, fund, path, row_count, total, created_at, verified)
             VALUES (?1,?2,?3,?4,?5,?6,1)",
            params![
                work_id,
                fund.key(),
                path.to_string_lossy(),
                report.row_count as i64,
                report.total,
                now(),
            ],
        )?;
        created.push(CreatedFile {
            fund_label: fund.label().to_string(),
            path: path.to_string_lossy().to_string(),
            row_count: report.row_count,
            total: report.total,
        });
    }

    crate::db::set_setting(conn, "last_output_dir", &dest_dir.to_string_lossy())?;
    touch_work(conn, work_id)?;
    Ok(GenerateResult { created, skipped })
}

#[cfg(test)]
#[path = "work_tests.rs"]
mod work_tests;

/// 견적서 하나가 어느 작업의 어떤 파일인지 (진행 알림에 쓴다)
pub fn quote_source_info(conn: &Connection, quote_id: i64) -> AppResult<(i64, String, bool)> {
    let (work_id, name, path): (i64, String, String) = conn.query_row(
        "SELECT work_id, source_name, source_path FROM work_quote WHERE id = ?1",
        [quote_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let slow = QuoteFormat::from_path(Path::new(&path)).is_slow();
    Ok((work_id, name, slow))
}

/// 견적서의 원본 파일 경로 (P4-2 원본 보기 · 원본 열기)
pub fn quote_source_path(conn: &Connection, quote_id: i64) -> AppResult<String> {
    Ok(conn.query_row(
        "SELECT source_path FROM work_quote WHERE id = ?1",
        [quote_id],
        |r| r.get(0),
    )?)
}
