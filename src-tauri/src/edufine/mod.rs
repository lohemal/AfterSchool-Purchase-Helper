//! P1-10 에듀파인 품의 XLSX 생성 (설계안 10장, P0-1 에서 검증한 사양).
//!
//! 사양은 **한곳에만** 있다. C5(나머지 세 재원의 구조)가 확정되면 이 파일만 고치면 된다.
//! 지금은 네 재원 모두 같은 `품목내역` 4열 구조로 만든다.
//!
//! 저장 절차: 임시 파일 → **다시 읽어 검증** → 사용자가 고른 폴더로 옮기기.

pub mod xlsx_spec;

use std::path::{Path, PathBuf};

use rust_xlsxwriter::{Format, FormatAlign, FormatBorder, Workbook};

use crate::domain::allocation::PumuiRow;
use crate::error::{AppError, AppResult};
use crate::edufine::xlsx_spec::{read_book, CellValue};

// ---------------------------------------------------------------- 사양 (한곳)

/// 에듀파인이 요구하는 시트 이름. **여기 한 곳에만 둔다.**
pub const SHEET_NAME: &str = "품목내역";
pub const HEADERS: [&str; 4] = ["내용", "규격", "수량", "예상단가"];
/// 규격은 늘 '식'
pub const SPEC_TEXT: &str = "식";
/// 수량은 늘 1
pub const QTY: f64 = 1.0;

/// 샘플에서 확인한 화면 서식 (P0-1)
const FONT: &str = "Dotum";
const FONT_SIZE: f64 = 9.0;
const HEADER_FONT_COLOR: u32 = 0xFFFFFF;
const HEADER_FILL: u32 = 0x000000;
const HEADER_BORDER_COLOR: u32 = 0xFF0000;
/// 열 너비는 라이브러리 한계로 샘플 값(44.5 / 12.25)에 정확히 닿지 못한다.
/// 가장 가까운 픽셀 값을 쓴다 (P0-1 에서 80~330 전수 확인).
const COL_A_PIXELS: u32 = 311; // → 44.43 (샘플 44.5)
const COL_BCD_PIXELS: u32 = 86; // → 12.29 (샘플 12.25)

fn header_format(first: bool) -> Format {
    let f = Format::new()
        .set_bold()
        .set_font_color(HEADER_FONT_COLOR)
        .set_background_color(HEADER_FILL)
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter)
        .set_text_wrap()
        .set_font_name(FONT)
        .set_font_size(FONT_SIZE)
        .set_border_right(FormatBorder::Thin)
        .set_border_top(FormatBorder::Thin)
        .set_border_bottom(FormatBorder::Thin)
        .set_border_color(HEADER_BORDER_COLOR);
    // 샘플은 첫 머리글 칸에만 왼쪽 선이 있다
    if first {
        f.set_border_left(FormatBorder::Thin)
    } else {
        f
    }
}

fn text_format() -> Format {
    Format::new()
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Left)
        .set_align(FormatAlign::VerticalCenter)
        .set_text_wrap()
        .set_font_name(FONT)
        .set_font_size(FONT_SIZE)
}

fn number_format() -> Format {
    Format::new()
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Right)
        .set_align(FormatAlign::VerticalCenter)
        .set_text_wrap()
        .set_num_format_index(3) // #,##0
        .set_font_name(FONT)
        .set_font_size(FONT_SIZE)
}

// ---------------------------------------------------------------- 생성

/// 품의 파일 한 개를 만든다. **행이 없으면 만들지 않는다** (설계안 14장 14번).
pub fn write_file(path: &Path, rows: &[PumuiRow]) -> AppResult<()> {
    if rows.is_empty() {
        return Err(AppError::new(
            "EDUFINE_NO_ROWS",
            "대상 금액이 없어 파일을 만들지 않았습니다.",
        ));
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }

    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.set_name(SHEET_NAME)
        .map_err(|e| AppError::new("EDUFINE_SHEET", "시트 이름을 정하지 못했습니다.").detail(e.to_string()))?;

    let head_first = header_format(true);
    let head_rest = header_format(false);
    let text = text_format();
    let num = number_format();

    for (c, h) in HEADERS.iter().enumerate() {
        let f = if c == 0 { &head_first } else { &head_rest };
        ws.write_string_with_format(0, c as u16, *h, f).map_err(xl)?;
    }
    for (i, row) in rows.iter().enumerate() {
        let r = (i + 1) as u32;
        ws.write_string_with_format(r, 0, &row.content, &text).map_err(xl)?;
        ws.write_string_with_format(r, 1, SPEC_TEXT, &text).map_err(xl)?;
        ws.write_number_with_format(r, 2, QTY, &num).map_err(xl)?;
        ws.write_number_with_format(r, 3, row.amount as f64, &num).map_err(xl)?;
    }
    ws.set_column_width_pixels(0, COL_A_PIXELS).map_err(xl)?;
    for c in 1..=3u16 {
        ws.set_column_width_pixels(c, COL_BCD_PIXELS).map_err(xl)?;
    }

    wb.save(path).map_err(xl)?;
    Ok(())
}

fn xl(e: rust_xlsxwriter::XlsxError) -> AppError {
    AppError::new("EDUFINE_WRITE", "품의 파일을 만들지 못했습니다.").detail(e.to_string())
}

// ---------------------------------------------------------------- 되읽어 검증

#[derive(Debug, Clone)]
pub struct VerifyReport {
    pub row_count: usize,
    pub total: i64,
    pub path: PathBuf,
}

/// 만든 파일을 **다시 읽어** 사양대로인지 본다. 하나라도 어긋나면 오류다.
pub fn verify_file(path: &Path, rows: &[PumuiRow]) -> AppResult<VerifyReport> {
    let book = read_book(path);

    if book.sheet_names != vec![SHEET_NAME.to_string()] {
        return Err(AppError::new(
            "EDUFINE_VERIFY",
            format!("시트 이름이 '{SHEET_NAME}' 하나여야 합니다."),
        )
        .detail(format!("{:?}", book.sheet_names)));
    }
    let sheet = &book.sheets[0];

    if !sheet.merged.is_empty() {
        return Err(AppError::new("EDUFINE_VERIFY", "병합된 칸이 있으면 안 됩니다."));
    }
    if sheet.cells.iter().any(|c| c.formula.is_some()) {
        return Err(AppError::new("EDUFINE_VERIFY", "수식이 있으면 안 됩니다."));
    }
    if !sheet.hidden_rows.is_empty() || !sheet.hidden_cols.is_empty() {
        return Err(AppError::new("EDUFINE_VERIFY", "숨겨진 줄이나 열이 있으면 안 됩니다."));
    }

    let cell = |r: &str| sheet.cell(r);
    for (c, h) in HEADERS.iter().enumerate() {
        let reference = format!("{}1", (b'A' + c as u8) as char);
        let got = cell(&reference).ok_or_else(|| verify_err(&reference, "머리글이 없습니다"))?;
        if got.value != CellValue::Text(h.to_string()) {
            return Err(verify_err(&reference, &format!("머리글이 '{h}' 이어야 합니다")));
        }
    }

    let mut total = 0i64;
    for (i, row) in rows.iter().enumerate() {
        let r = i + 2;
        let a = cell(&format!("A{r}")).ok_or_else(|| verify_err(&format!("A{r}"), "내용이 없습니다"))?;
        if a.value != CellValue::Text(row.content.clone()) {
            return Err(verify_err(&format!("A{r}"), "내용이 다릅니다"));
        }
        let b = cell(&format!("B{r}")).ok_or_else(|| verify_err(&format!("B{r}"), "규격이 없습니다"))?;
        if b.value != CellValue::Text(SPEC_TEXT.to_string()) {
            return Err(verify_err(&format!("B{r}"), "규격이 '식' 이어야 합니다"));
        }
        let c = cell(&format!("C{r}")).ok_or_else(|| verify_err(&format!("C{r}"), "수량이 없습니다"))?;
        if c.value != CellValue::Number("1".to_string()) {
            return Err(verify_err(&format!("C{r}"), "수량이 숫자 1 이어야 합니다"));
        }
        if c.num_fmt != "#,##0" {
            return Err(verify_err(&format!("C{r}"), "수량 표시 형식이 다릅니다"));
        }
        let d = cell(&format!("D{r}")).ok_or_else(|| verify_err(&format!("D{r}"), "예상단가가 없습니다"))?;
        if d.value != CellValue::Number(row.amount.to_string()) {
            return Err(verify_err(&format!("D{r}"), "예상단가가 다릅니다"));
        }
        if d.num_fmt != "#,##0" {
            return Err(verify_err(&format!("D{r}"), "예상단가 표시 형식이 다릅니다"));
        }
        total += row.amount;
    }

    // 행 수가 정확해야 한다 (빈 행·합계 행이 붙으면 안 된다).
    // **주의**: `reference.ends_with('1')` 로 머리글을 거르면 11행·21행도 함께 빠진다.
    // 글자를 떼고 숫자만 읽어야 한다.
    let row_no = |reference: &str| -> usize {
        reference.chars().filter(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap_or(0)
    };
    let data_cells = sheet.cells.iter().filter(|c| row_no(&c.reference) > 1).count();
    let want = rows.len() * 4;
    if data_cells != want {
        return Err(AppError::new("EDUFINE_VERIFY", "만든 파일의 줄 수가 맞지 않습니다.")
            .detail(format!("칸 {data_cells} / 기대 {want}")));
    }
    let max_row = sheet.cells.iter().map(|c| row_no(&c.reference)).max().unwrap_or(0);
    if max_row != rows.len() + 1 {
        return Err(AppError::new("EDUFINE_VERIFY", "만든 파일에 여분의 줄이 있습니다.")
            .detail(format!("마지막 줄 {max_row} / 기대 {}", rows.len() + 1)));
    }
    if sheet
        .cells
        .iter()
        .any(|c| matches!(&c.value, CellValue::Text(t) if crate::settlement::is_total_label(t)))
    {
        return Err(AppError::new("EDUFINE_VERIFY", "합계 줄이 들어가면 안 됩니다."));
    }

    Ok(VerifyReport { row_count: rows.len(), total, path: path.to_path_buf() })
}

fn verify_err(reference: &str, what: &str) -> AppError {
    AppError::new("EDUFINE_VERIFY", format!("만든 파일이 사양과 다릅니다: {reference} {what}"))
}

/// 이미 있는 파일을 **덮어쓰지 않는다.** 같은 이름이 있으면 `… (2).xlsx` 로 비켜 간다.
///
/// 품의를 다시 만드는 일이 잦은데, 먼저 만든 파일을 말없이 지우면
/// 이미 결재에 올린 자료를 잃을 수 있다. 자리를 비켜 주고 어디에 만들었는지 알려 준다.
pub fn free_path(dest: &Path) -> PathBuf {
    if !dest.exists() {
        return dest.to_path_buf();
    }
    let dir = dest.parent().unwrap_or_else(|| Path::new("."));
    let stem = dest.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let ext = dest.extension().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    for n in 2..1000 {
        let name =
            if ext.is_empty() { format!("{stem} ({n})") } else { format!("{stem} ({n}).{ext}") };
        let p = dir.join(name);
        if !p.exists() {
            return p;
        }
    }
    dest.to_path_buf()
}

/// 만들고 검증한 뒤 옮긴다. 검증에 실패하면 **결과 파일을 남기지 않는다.**
///
/// `dest` 에 이미 파일이 있으면 덮어쓰지 않고 옆 이름으로 만든다.
/// 실제로 만든 자리는 `VerifyReport::path` 에 담아 돌려준다.
pub fn create_verified(dest: &Path, rows: &[PumuiRow]) -> AppResult<VerifyReport> {
    let tmp_dir = std::env::temp_dir().join("purchase-helper-out");
    std::fs::create_dir_all(&tmp_dir)?;
    // 임시 파일 이름은 **반드시 서로 달라야** 한다. 시각만 쓰면 같은 밀리초에 두 개가 겹쳐
    // 한쪽이 다른 쪽의 파일을 지운다(시험이 병렬로 돌 때 실제로 겪었다).
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let tmp: PathBuf = tmp_dir.join(format!(
        "edufine-{}-{}-{}.xlsx",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed),
        chrono::Local::now().format("%H%M%S%3f")
    ));

    let made = (|| -> AppResult<VerifyReport> {
        write_file(&tmp, rows)?;
        verify_file(&tmp, rows)
    })();

    let report = match made {
        Ok(r) => r,
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }
    };

    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // **있는 파일을 덮어쓰지 않는다** — 자리를 비켜 만든다
    let final_path = free_path(dest);
    // rename 은 드라이브가 다르면 실패한다 → 복사 후 지우기로 대체
    if std::fs::rename(&tmp, &final_path).is_err() {
        std::fs::copy(&tmp, &final_path)?;
        let _ = std::fs::remove_file(&tmp);
    }
    Ok(VerifyReport { path: final_path, ..report })
}

/// 파일 이름. C6 이 정해지면 이 함수만 고친다.
pub fn file_name(school_year: &str, month: &str, kind: &str, fund_label: &str) -> String {
    let parts: Vec<&str> = [school_year, month, kind, fund_label]
        .into_iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    format!("{}.xlsx", parts.join("_"))
}

#[cfg(test)]
#[path = "edufine_tests.rs"]
mod edufine_tests;
