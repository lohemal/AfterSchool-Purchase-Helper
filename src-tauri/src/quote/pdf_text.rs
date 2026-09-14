//! P2 텍스트 PDF → 문자 + 좌표 → 공통 좌표 표 복원(`layout.rs`) → `RawTable`.
//!
//! **확장자만 보고 텍스트 PDF 라고 단정하지 않는다.** 먼저 글자를 뽑아 보고
//! 표를 되살릴 만큼 나오는지 재 본 뒤에야 이 경로를 쓴다(`probe`).
//! 모자라면 스캔본으로 보고 P3 OCR 경로로 넘긴다.

use std::path::Path;

use pdf_extract::{output_doc, Document, MediaBox, OutputDev, OutputError, Transform};

use crate::error::{AppError, AppResult};
use crate::quote::layout::{self, Block};
use crate::quote::model::RawTable;

/// 한 글자와 그 위치
struct Glyph {
    text: String,
    x: f64,
    y: f64,
    width: f64,
    size: f64,
    page: u32,
}

/// `output_doc` 가 불러 주는 수집기. 글자마다 위치를 받아 쌓는다.
#[derive(Default)]
struct Collector {
    glyphs: Vec<Glyph>,
    page: u32,
    /// 쪽 높이 — PDF 는 y 가 아래에서 위로 자란다. 화면 차례대로 보려고 뒤집는다.
    page_top: f64,
}

impl OutputDev for Collector {
    fn begin_page(
        &mut self,
        page_num: u32,
        media_box: &MediaBox,
        _art_box: Option<(f64, f64, f64, f64)>,
    ) -> Result<(), OutputError> {
        self.page = page_num;
        self.page_top = media_box.ury;
        Ok(())
    }

    fn end_page(&mut self) -> Result<(), OutputError> {
        Ok(())
    }

    fn output_character(
        &mut self,
        trm: &Transform,
        width: f64,
        _spacing: f64,
        font_size: f64,
        char: &str,
    ) -> Result<(), OutputError> {
        // 변환 행렬의 이동 성분이 글자의 위치다
        let x = trm.m31;
        let y = trm.m32;
        // 글자 크기도 행렬에 배율이 섞여 있다
        let scale = (trm.m11.powi(2) + trm.m12.powi(2)).sqrt();
        let size = if scale > 0.0 { font_size * scale } else { font_size };
        self.glyphs.push(Glyph {
            text: char.to_string(),
            x,
            // 위에서 아래로 세는 좌표로 바꾼다
            y: self.page_top - y,
            width: width * size.max(0.1),
            size: size.max(0.1),
            page: self.page,
        });
        Ok(())
    }

    fn begin_word(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
    fn end_word(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
    fn end_line(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
}

/// PDF 에서 글자를 뽑는다. 쪽마다 덩어리 목록을 돌려준다.
pub fn collect(path: &Path) -> AppResult<Vec<(u32, Vec<Block>)>> {
    let doc = Document::load(path).map_err(|e| {
        AppError::new("PDF_OPEN_FAILED", "PDF 를 열지 못했습니다. 파일이 손상되었거나 암호가 걸려 있을 수 있습니다. 품목을 직접 입력할 수 있습니다.").detail(e.to_string())
    })?;

    let mut c = Collector::default();
    // 글자가 하나도 없는 PDF 에서도 오류로 끝나지 않게 한다 (스캔본일 수 있다)
    if let Err(e) = output_doc(&doc, &mut c) {
        log::warn!("PDF 글자 추출 중 문제: {e}");
    }

    let mut by_page: std::collections::BTreeMap<u32, Vec<Block>> = Default::default();
    for g in c.glyphs {
        if g.text.trim().is_empty() {
            continue;
        }
        by_page.entry(g.page).or_default().push(Block {
            text: g.text,
            x0: g.x,
            x1: g.x + g.width.max(g.size * 0.3),
            y0: g.y - g.size,
            y1: g.y,
            origin: format!("{}쪽", g.page),
        });
    }
    Ok(by_page.into_iter().collect())
}

/// 텍스트 PDF 로 볼 수 있는지 재 본 결과
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PdfKind {
    /// 표를 되살릴 만큼 글자가 있다 → P2 경로
    Text,
    /// 글자가 거의 없거나 표를 못 만든다 → P3 OCR 후보
    NeedsOcr(String),
}

/// 견적서 표를 되살릴 만큼의 글자가 있는지 본다.
///
/// **글자가 조금 나온다는 이유만으로 텍스트 PDF 라고 하지 않는다.**
/// 열 이름 낱말과 숫자가 함께 충분히 있어야 한다.
pub fn probe(path: &Path) -> AppResult<PdfKind> {
    let pages = collect(path)?;
    let total: usize = pages.iter().map(|(_, b)| b.len()).sum();
    if total < 20 {
        log::info!("PDF 글자 조각 {total}개 — 스캔본으로 본다");
        return Ok(PdfKind::NeedsOcr(
            "PDF 안에 글자가 없어 스캔한 문서로 보고 사진처럼 읽었습니다.".into(),
        ));
    }

    for (_, blocks) in &pages {
        let merged = layout::merge_adjacent(blocks.clone(), guess_gap(blocks));
        let rows = layout::group_rows(&merged);
        let Some((_, hits)) = layout::find_header_row(&rows) else { continue };
        // 열 이름이 3개 이상 맞고 숫자 칸도 있어야 표로 본다
        let numbers = merged
            .iter()
            .filter(|b| crate::quote::number::looks_numeric(&b.text))
            .count();
        if hits >= 3 && numbers >= 4 {
            return Ok(PdfKind::Text);
        }
    }
    Ok(PdfKind::NeedsOcr(
        "PDF 의 글자로는 견적서 표를 알아보지 못해 사진처럼 읽었습니다.".into(),
    ))
}

/// 글자 사이를 얼마나 벌어져도 한 낱말로 볼지 — 글자 크기에서 짐작한다.
pub fn guess_gap(blocks: &[Block]) -> f64 {
    let mut sizes: Vec<f64> = blocks.iter().map(|b| b.height()).filter(|h| *h > 0.0).collect();
    if sizes.is_empty() {
        return 6.0;
    }
    sizes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    sizes[sizes.len() / 2] * 0.9
}

/// 텍스트 PDF 를 읽어 표를 돌려준다. 쪽마다 표 하나로 본다.
pub fn read(path: &Path) -> AppResult<Vec<RawTable>> {
    let pages = collect(path)?;
    let mut tables = Vec::new();

    for (page, blocks) in pages {
        let gap = guess_gap(&blocks);
        // 쪽 제목은 위쪽 글자에서 찾는다 (견적서 / 납품서 구분에 쓴다)
        let title = page_title(&blocks, page);
        if let Some(t) = layout::to_raw_table(blocks, &title, gap) {
            tables.push(t);
        }
    }

    if tables.is_empty() {
        return Err(AppError::new(
            "PDF_NO_TABLE",
            "PDF 에서 견적서 표를 찾지 못했습니다. 품목을 직접 입력해 주세요.",
        ));
    }
    Ok(tables)
}

/// 쪽 위쪽 글자를 모아 제목으로 쓴다
fn page_title(blocks: &[Block], page: u32) -> String {
    let mut top: Vec<&Block> = blocks.iter().collect();
    top.sort_by(|a, b| a.y0.partial_cmp(&b.y0).unwrap_or(std::cmp::Ordering::Equal));
    let head: String = top.iter().take(14).map(|b| b.text.as_str()).collect::<Vec<_>>().join("");
    if head.trim().is_empty() {
        format!("{page}쪽")
    } else {
        head
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gap_from_glyph_size() {
        let blocks = vec![
            Block { text: "가".into(), x0: 0.0, x1: 10.0, y0: 0.0, y1: 10.0, origin: String::new() },
            Block { text: "나".into(), x0: 0.0, x1: 10.0, y0: 0.0, y1: 12.0, origin: String::new() },
        ];
        let g = guess_gap(&blocks);
        assert!(g > 8.0 && g < 12.0, "{g}");
        assert_eq!(guess_gap(&[]), 6.0);
    }

    #[test]
    fn title_from_top_glyphs() {
        let blocks = vec![
            Block { text: "견".into(), x0: 0.0, x1: 5.0, y0: 5.0, y1: 15.0, origin: String::new() },
            Block { text: "적".into(), x0: 6.0, x1: 11.0, y0: 5.0, y1: 15.0, origin: String::new() },
            Block { text: "서".into(), x0: 12.0, x1: 17.0, y0: 5.0, y1: 15.0, origin: String::new() },
            Block { text: "품".into(), x0: 0.0, x1: 5.0, y0: 90.0, y1: 100.0, origin: String::new() },
        ];
        assert!(page_title(&blocks, 1).contains("견적서"));
    }
}
