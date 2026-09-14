//! XLSX / XLS / XLSM 견적서 → RawTable (P0-2 에서 검증한 방식).

use std::path::Path;

use calamine::{open_workbook_auto, Data, Reader};

use crate::error::{AppError, AppResult};
use crate::quote::model::{RawCell, RawTable};

fn cell_text(d: Option<&Data>) -> String {
    match d {
        Some(Data::String(s)) => s.trim().to_string(),
        Some(Data::Float(f)) => {
            if f.fract().abs() < f64::EPSILON {
                format!("{}", *f as i64)
            } else {
                f.to_string()
            }
        }
        Some(Data::Int(i)) => i.to_string(),
        Some(Data::Bool(b)) => b.to_string(),
        Some(Data::DateTime(dt)) => dt.to_string(),
        Some(Data::DurationIso(s)) | Some(Data::DateTimeIso(s)) => s.clone(),
        Some(Data::Error(_)) | Some(Data::Empty) | None => String::new(),
    }
}

/// `0` → `A`, `26` → `AA`
pub fn col_letter(mut c: u32) -> String {
    let mut out = String::new();
    loop {
        out.insert(0, (b'A' + (c % 26) as u8) as char);
        if c < 26 {
            break;
        }
        c = c / 26 - 1;
    }
    out
}

/// 시트마다 표 하나로 본다. 시트가 여럿이면 표도 여럿이 된다.
pub fn read(path: &Path) -> AppResult<Vec<RawTable>> {
    let mut wb = open_workbook_auto(path).map_err(|e| {
        AppError::new("QUOTE_OPEN_FAILED", "견적서 파일을 열지 못했습니다. 파일이 손상되었거나 다른 프로그램이 쓰고 있을 수 있습니다. 품목을 직접 입력할 수 있습니다.").detail(e.to_string())
    })?;

    let names = wb.sheet_names().to_vec();
    let mut tables = Vec::new();

    for name in names {
        let Ok(range) = wb.worksheet_range(&name) else { continue };
        if range.is_empty() {
            continue;
        }
        // **calamine 의 Range 는 A1 이 아니라 '실제로 쓴 영역' 에서 시작한다** (P0-2 실측).
        // 원본 셀 주소를 보여주려면 이 값을 더해야 한다.
        let (row0, col0) = range.start().unwrap_or((0, 0));

        let (h, w) = (range.height(), range.width());
        let mut rows = Vec::with_capacity(h);
        for r in 0..h {
            let mut row = Vec::with_capacity(w);
            for c in 0..w {
                let text = cell_text(range.get((r, c)));
                let cell_ref = format!(
                    "{}!{}{}",
                    name,
                    col_letter(col0 + c as u32),
                    row0 + r as u32 + 1
                );
                row.push(RawCell::new(text, cell_ref));
            }
            rows.push(row);
        }

        tables.push(RawTable { title: name.clone(), rows, loose_text: Vec::new() });
    }

    if tables.is_empty() {
        return Err(AppError::new(
            "QUOTE_EMPTY",
            "견적서에서 읽을 수 있는 시트를 찾지 못했습니다. 품목을 직접 입력할 수 있습니다.",
        ));
    }
    Ok(tables)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_letters() {
        assert_eq!(col_letter(0), "A");
        assert_eq!(col_letter(1), "B");
        assert_eq!(col_letter(25), "Z");
        assert_eq!(col_letter(26), "AA");
        assert_eq!(col_letter(27), "AB");
        assert_eq!(col_letter(51), "AZ");
        assert_eq!(col_letter(52), "BA");
    }
}
