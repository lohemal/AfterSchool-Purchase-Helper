//! 화면이 부르는 명령들. 이름은 프런트의 `src/ipc/*.ts` 와 1:1 이다.

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::domain::allocation::DeptAllocation;
use crate::domain::setup::{Department, SetupExport};
use crate::domain::status::GateResult;
use crate::domain::work::{
    CheckRow, FundPreview, GenerateResult, ItemEdit, QuoteRow, SettlementRowView, Work,
};
use crate::domain::{CompareBasis, Funds};
use crate::error::AppError;
use crate::AppState;

type R<T> = Result<T, AppError>;

// ---------------------------------------------------------------- 설정

#[tauri::command]
fn list_departments(state: State<AppState>) -> R<Vec<Department>> {
    state.db.read(crate::domain::setup::list_departments)
}

#[tauri::command]
fn save_department(state: State<AppState>, dept: Department) -> R<i64> {
    state.db.write(|c| crate::domain::setup::upsert_department(c, &dept))
}

#[tauri::command]
fn delete_department(state: State<AppState>, id: i64) -> R<()> {
    state.db.write(|c| crate::domain::setup::delete_department(c, id))
}

#[tauri::command]
fn export_setup(state: State<AppState>) -> R<SetupExport> {
    state.db.read(crate::domain::setup::export_setup)
}

#[tauri::command]
fn import_setup(state: State<AppState>, data: SetupExport) -> R<usize> {
    state.db.write(|c| crate::domain::setup::import_setup(c, &data))
}

// ---------------------------------------------------------------- 작업

#[tauri::command]
fn list_works(state: State<AppState>) -> R<Vec<Work>> {
    state.db.read(crate::domain::work::list_works)
}

#[tauri::command]
fn create_work(
    state: State<AppState>,
    title: String,
    school_year: String,
    month: String,
    kind: String,
) -> R<i64> {
    state
        .db
        .write(|c| crate::domain::work::create_work(c, &title, &school_year, &month, &kind))
}

/// 작업 이름만 바꾼다. 작업 id 와 딸린 자료는 그대로다.
#[tauri::command]
fn rename_work(state: State<AppState>, id: i64, title: String) -> R<()> {
    state.db.write(|c| crate::domain::work::rename_work(c, id, &title))
}

#[tauri::command]
fn delete_work(state: State<AppState>, id: i64) -> R<()> {
    state.db.write(|c| crate::domain::work::delete_work(c, id))
}

#[tauri::command]
fn get_work(state: State<AppState>, id: i64) -> R<Work> {
    state.db.read(|c| crate::domain::work::get_work(c, id))
}

// ---------------------------------------------------------------- 견적서

/// 견적서를 읽는 동안 화면에 알려 주는 진행 상황.
///
/// 사진·스캔 PDF 는 한 장에 20초쯤 걸린다. 아무 표시도 없으면 멈춘 것처럼 보이므로
/// 파일마다 시작·끝을 알린다. (`stage`: `start` | `done` | `failed`)
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct QuoteProgress {
    work_id: i64,
    index: usize,
    total: usize,
    name: String,
    /// 사진·PDF 처럼 오래 걸리는 파일인가
    slow: bool,
    stage: &'static str,
    message: String,
}

/// 화면이 듣는 이름. `src/ipc/index.ts` 와 같아야 한다.
const QUOTE_PROGRESS: &str = "quote-progress";

#[tauri::command]
fn register_quotes(
    app: AppHandle,
    state: State<AppState>,
    work_id: i64,
    paths: Vec<String>,
) -> R<Vec<String>> {
    let mut problems = Vec::new();
    let total = paths.len();
    for (i, p) in paths.into_iter().enumerate() {
        let path = PathBuf::from(&p);
        let name = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or(p.clone());
        let slow = crate::quote::model::QuoteFormat::from_path(&path).is_slow();

        let tell = |stage: &'static str, message: String| {
            let _ = app.emit(
                QUOTE_PROGRESS,
                QuoteProgress {
                    work_id,
                    index: i + 1,
                    total,
                    name: name.clone(),
                    slow,
                    stage,
                    message,
                },
            );
        };

        tell(
            "start",
            if slow {
                "글자를 알아보는 중입니다. 한 장에 20초쯤 걸립니다.".into()
            } else {
                "읽는 중입니다.".into()
            },
        );

        let r = state
            .db
            .write(|c| crate::domain::work::register_quote(c, work_id, &path));
        match r {
            Ok(_) => tell("done", String::new()),
            Err(e) => {
                tell("failed", e.message.clone());
                problems.push(format!("{name}: {}", e.message));
            }
        }
    }
    Ok(problems)
}

#[tauri::command]
fn list_quotes(state: State<AppState>, work_id: i64) -> R<Vec<QuoteRow>> {
    state.db.read(|c| crate::domain::work::load_quotes(c, work_id))
}

#[tauri::command]
fn set_quote_vendor(state: State<AppState>, quote_id: i64, vendor_unit_id: Option<i64>) -> R<()> {
    state
        .db
        .write(|c| crate::domain::work::set_quote_vendor(c, quote_id, vendor_unit_id))
}

/// P4-2 — 사진·스캔 PDF 의 **원본 그림**을 화면에서 볼 수 있게 준비한다.
/// 원본 파일은 읽기만 한다.
#[tauri::command]
fn quote_preview(state: State<AppState>, quote_id: i64) -> R<crate::quote::preview::QuotePreview> {
    let path = state.db.read(|c| crate::domain::work::quote_source_path(c, quote_id))?;
    crate::quote::preview::build(std::path::Path::new(&path))
}

#[tauri::command]
fn delete_quote(state: State<AppState>, quote_id: i64) -> R<()> {
    state.db.write(|c| crate::domain::work::delete_quote(c, quote_id))
}

#[tauri::command]
fn reparse_quote(app: AppHandle, state: State<AppState>, quote_id: i64) -> R<()> {
    // 사진·스캔 PDF 는 다시 읽는 데도 20초쯤 걸린다 → 같은 진행 알림을 쓴다
    let (work_id, name, slow) = state
        .db
        .read(|c| crate::domain::work::quote_source_info(c, quote_id))
        .unwrap_or((0, String::new(), false));
    let tell = |stage: &'static str, message: String| {
        let _ = app.emit(
            QUOTE_PROGRESS,
            QuoteProgress {
                work_id,
                index: 1,
                total: 1,
                name: name.clone(),
                slow,
                stage,
                message,
            },
        );
    };
    tell("start", if slow { "글자를 다시 알아보는 중입니다.".into() } else { "다시 읽는 중입니다.".into() });
    let r = state.db.write(|c| crate::domain::work::reparse_quote(c, quote_id));
    match &r {
        Ok(()) => tell("done", String::new()),
        Err(e) => tell("failed", e.message.clone()),
    }
    r
}

#[tauri::command]
fn update_item(state: State<AppState>, edit: ItemEdit) -> R<()> {
    state.db.write(|c| crate::domain::work::update_item(c, &edit).map(|_| ()))
}

#[tauri::command]
fn add_item(state: State<AppState>, quote_id: i64) -> R<i64> {
    state.db.write(|c| crate::domain::work::add_item(c, quote_id))
}

#[tauri::command]
fn delete_item(state: State<AppState>, item_id: i64) -> R<()> {
    state.db.write(|c| crate::domain::work::delete_item(c, item_id).map(|_| ()))
}

#[tauri::command]
fn set_representative(state: State<AppState>, quote_id: i64, item_id: Option<i64>) -> R<()> {
    state
        .db
        .write(|c| crate::domain::work::set_representative(c, quote_id, item_id))
}

#[tauri::command]
fn set_phrase_override(state: State<AppState>, quote_id: i64, text: Option<String>) -> R<()> {
    state
        .db
        .write(|c| crate::domain::work::set_phrase_override(c, quote_id, text.as_deref()))
}

#[tauri::command]
fn set_compare_basis(state: State<AppState>, quote_id: i64, basis: String) -> R<()> {
    let b = CompareBasis::from_key(&basis)
        .ok_or_else(|| AppError::new("BAD_BASIS", "비교 기준이 올바르지 않습니다."))?;
    state
        .db
        .write(|c| crate::domain::work::set_compare_basis(c, quote_id, b))
}

// ---------------------------------------------------------------- 정산자료

#[tauri::command]
fn load_settlement(state: State<AppState>, work_id: i64, path: String) -> R<Vec<crate::domain::Warning>> {
    state
        .db
        .write(|c| crate::domain::work::load_settlement(c, work_id, &PathBuf::from(&path)))
}

#[tauri::command]
fn list_settlement(state: State<AppState>, work_id: i64) -> R<Vec<SettlementRowView>> {
    state.db.read(|c| crate::domain::work::settlement_rows(c, work_id))
}

#[tauri::command]
fn map_alias(
    state: State<AppState>,
    work_id: i64,
    source_name: String,
    department_id: i64,
) -> R<()> {
    state
        .db
        .write(|c| crate::domain::work::map_alias(c, work_id, &source_name, department_id))
}

// ---------------------------------------------------------------- 배분·검증

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeptAllocationView {
    department_id: i64,
    department_name: String,
    settlement: Funds,
    has_settlement: bool,
    vendors: Vec<VendorAllocationView>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VendorAllocationView {
    vendor_unit_id: i64,
    mgmt_name: String,
    allocated: Funds,
    missing: bool,
    quote_total: Option<i64>,
    /// 거래처가 하나여서 값을 고칠 수 없는가
    locked: bool,
}

fn to_view(d: &DeptAllocation) -> DeptAllocationView {
    let single = d.vendors.len() == 1;
    DeptAllocationView {
        department_id: d.department_id,
        department_name: d.department_name.clone(),
        settlement: d.settlement,
        has_settlement: d.has_settlement,
        vendors: d
            .vendors
            .iter()
            .map(|v| VendorAllocationView {
                vendor_unit_id: v.vendor_unit_id,
                mgmt_name: v.mgmt_name.clone(),
                allocated: v.allocated,
                missing: v.missing,
                quote_total: v.quote_total,
                locked: single && d.has_settlement,
            })
            .collect(),
    }
}

#[tauri::command]
fn list_allocations(state: State<AppState>, work_id: i64) -> R<Vec<DeptAllocationView>> {
    state.db.write(|c| {
        crate::domain::work::apply_auto_allocations(c, work_id)?;
        Ok(crate::domain::work::dept_allocations(c, work_id)?.iter().map(to_view).collect())
    })
}

#[tauri::command]
fn save_allocation(
    state: State<AppState>,
    work_id: i64,
    vendor_unit_id: i64,
    funds: Funds,
) -> R<()> {
    state
        .db
        .write(|c| crate::domain::work::save_allocation(c, work_id, vendor_unit_id, funds, false))
}

#[tauri::command]
fn run_checks(state: State<AppState>, work_id: i64) -> R<Vec<CheckRow>> {
    state.db.write(|c| {
        crate::domain::work::run_checks(c, work_id)?;
        crate::domain::work::list_checks(c, work_id)
    })
}

#[tauri::command]
fn acknowledge_check(state: State<AppState>, check_id: i64, reason: String) -> R<()> {
    state.db.write(|c| crate::domain::work::acknowledge(c, check_id, &reason))
}

#[tauri::command]
fn unacknowledge_check(state: State<AppState>, check_id: i64) -> R<()> {
    state.db.write(|c| crate::domain::work::unacknowledge(c, check_id))
}

#[tauri::command]
fn generation_gate(state: State<AppState>, work_id: i64) -> R<GateResult> {
    state.db.read(|c| crate::domain::work::gate(c, work_id))
}

// ---------------------------------------------------------------- 미리보기·생성

#[tauri::command]
fn preview(state: State<AppState>, work_id: i64) -> R<Vec<FundPreview>> {
    state.db.read(|c| crate::domain::work::preview(c, work_id))
}

#[tauri::command]
fn generate(state: State<AppState>, work_id: i64, dest_dir: String) -> R<GenerateResult> {
    state
        .db
        .write(|c| crate::domain::work::generate(c, work_id, &PathBuf::from(&dest_dir)))
}

#[tauri::command]
fn last_output_dir(state: State<AppState>) -> R<Option<String>> {
    state.db.read(|c| {
        Ok(crate::db::get_setting(c, "last_output_dir")?
            .filter(|d| std::path::Path::new(d).is_dir()))
    })
}

#[tauri::command]
fn hangul_available() -> bool {
    crate::quote::hwp::hangul_available()
}

/// 이 컴퓨터에서 사진·스캔 PDF 를 읽을 수 있는가 (Windows 한국어 글자 인식)
#[tauri::command]
fn ocr_available() -> bool {
    crate::quote::ocr::available()
}

pub fn handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        list_departments,
        save_department,
        delete_department,
        export_setup,
        import_setup,
        list_works,
        create_work,
        rename_work,
        delete_work,
        get_work,
        register_quotes,
        list_quotes,
        set_quote_vendor,
        delete_quote,
        quote_preview,
        reparse_quote,
        update_item,
        add_item,
        delete_item,
        set_representative,
        set_phrase_override,
        set_compare_basis,
        load_settlement,
        list_settlement,
        map_alias,
        list_allocations,
        save_allocation,
        run_checks,
        acknowledge_check,
        unacknowledge_check,
        generation_gate,
        preview,
        generate,
        last_output_dir,
        hangul_available,
        ocr_available,
    ]
}
