//! 견적서 읽기. 형식마다 `RawTable` 을 만들고 **공통 `table.rs` 하나**가 품목을 뽑는다.
//!
//! 형식이 갈리는 자리는 `read_tables` 하나뿐이다. 그 뒤부터는 XLSX·HWP·PDF·사진이 완전히 같은 길을 간다.

pub mod com;
pub mod hwp;
pub mod hwpx;
pub mod layout;
pub mod model;
pub mod number;
pub mod ocr;
pub mod pdf_scan;
pub mod pdf_text;
pub mod preview;
pub mod table;
pub mod trust;
pub mod xlsx;

use std::path::Path;

use crate::error::{AppError, AppResult};
use crate::quote::model::{ParsedQuote, QuoteFormat, RawTable};
use crate::quote::trust::Source;

/// 파일 하나를 읽어 표 목록과 "어느 경로로 읽었는지" 를 돌려준다.
///
/// PDF 는 **확장자만 보고 정하지 않는다.** 글자를 먼저 재 보고(`pdf_text::probe`)
/// 표를 되살릴 만큼 나오면 텍스트 경로, 아니면 그림을 꺼내 OCR 경로로 간다.
pub fn read_tables(path: &Path, format: QuoteFormat) -> AppResult<(Vec<RawTable>, Source, String)> {
    match format {
        QuoteFormat::Xlsx | QuoteFormat::Xls | QuoteFormat::Xlsm => {
            Ok((xlsx::read(path)?, Source::Structured, String::new()))
        }
        QuoteFormat::Hwpx => Ok((hwpx::read(path)?, Source::Structured, String::new())),
        QuoteFormat::Hwp => Ok((hwp::read(path)?, Source::Structured, String::new())),

        QuoteFormat::Pdf => match pdf_text::probe(path)? {
            pdf_text::PdfKind::Text => {
                Ok((pdf_text::read(path)?, Source::PdfText, "PDF 안의 글자로 읽었습니다.".into()))
            }
            pdf_text::PdfKind::NeedsOcr(why) => {
                // 텍스트가 모자라면 **실패로 끝내지 않고** 그림을 꺼내 OCR 로 넘긴다
                let tables = pdf_scan::read(path)?;
                Ok((tables, Source::Ocr, why))
            }
        },

        QuoteFormat::Image => {
            let tables = ocr::read_image(path, "사진")?;
            // 작은 사진은 읽히더라도 틀릴 수 있다 — 읽은 뒤에도 한 마디 남긴다
            let advice = image::open(path)
                .map(|i| ocr::resolution_advice(&i).trim().to_string())
                .unwrap_or_default();
            Ok((tables, Source::Ocr, format!("사진에서 읽었습니다. {advice}").trim().to_string()))
        }

        QuoteFormat::Unsupported => {
            Err(AppError::new("QUOTE_UNSUPPORTED", format.unsupported_message()))
        }
    }
}

/// 견적서 한 장을 끝까지 읽는다.
pub fn parse(path: &Path) -> AppResult<ParsedQuote> {
    let format = QuoteFormat::from_path(path);
    if format == QuoteFormat::Unsupported {
        return Err(AppError::new("QUOTE_UNSUPPORTED", format.unsupported_message()));
    }
    let (tables, source, note) = read_tables(path, format)?;

    // 표가 여럿이면 제목으로 고른다. 견적서와 납품서는 크기로 못 가른다(P0-5).
    let (idx, pick_note, warn) = table::pick_table(&tables);
    let mut q = table::interpret(&tables[idx]);
    q.table_note = if note.is_empty() { pick_note } else { format!("{note} {pick_note}") };
    if let Some(w) = warn {
        q.warnings.push(w);
    }

    // 사진·PDF 에서 읽었다면 근거 기반 신뢰도와 경고를 붙인다. **값은 바꾸지 않는다.**
    trust::apply(&mut q, source);
    q.source = source_key(source).to_string();
    q.trust = trust::summarize(&q, source).key().to_string();
    Ok(q)
}

pub fn source_key(s: Source) -> &'static str {
    match s {
        Source::Structured => "structured",
        Source::PdfText => "pdf_text",
        Source::Ocr => "ocr",
    }
}

#[cfg(test)]
#[path = "real_sample_tests.rs"]
mod real_sample_tests;
