//! P3 이미지 OCR — Windows 내장 OCR(`Windows.Media.Ocr`).
//!
//! 외부 서비스로 자료를 보내지 않는다. 추가 설치도 필요 없다(한국어 OCR 언어 팩만 있으면 된다).
//! P0-6 에서 검증한 방식을 그대로 옮겼다.
//!
//! **알아 둘 것 (P0-6 실측)**
//!   - 원본 크기로는 한글이 거의 다 깨졌다(품목명 0/4). **3배로 확대하면 4/4** 였다 → 확대는 필수다.
//!   - Windows OCR 은 **낱말 신뢰도를 주지 않는다.** 가짜 확률을 만들지 않고,
//!     검증식(수량×단가=금액 등)으로 믿을 만한지 판단한다(`quote/trust.rs`).
//!   - `₩` 가 붙은 합계는 3배에서도 깨진다 → 품목 합으로 대신하는 경로가 정식 경로다.

pub mod engine;

use std::path::Path;

use crate::error::{AppError, AppResult};
use crate::quote::layout::{self, Block};
use crate::quote::model::RawTable;

/// OCR 전 확대 배율. P0-6 실측에 따른 기본값이며 낮추지 않는다.
pub const SCALE: u32 = 3;
/// Windows OCR 이 받는 한 변의 최대 픽셀
const MAX_SIDE: u32 = 10_000;
/// 이보다 작은 사진은 3배로 키워도 글자가 뭉개진다.
///
/// P0-6 에서 통과한 실제 샘플이 794×1123(A4 를 96dpi 로 스캔한 크기)이므로
/// 그보다 낮은 선을 잡았다. **막지는 않고 안내만 한다.**
const SMALL_SIDE: u32 = 700;

/// 사진이 글자를 알아보기에 너무 작은가
pub fn looks_low_resolution(img: &image::DynamicImage) -> bool {
    img.width().max(img.height()) < SMALL_SIDE
}

/// 해상도가 낮을 때 덧붙일 한 마디 (아니면 빈 문자열)
pub fn resolution_advice(img: &image::DynamicImage) -> &'static str {
    if looks_low_resolution(img) {
        " 사진이 작아 글자가 뭉개졌을 수 있습니다. 더 크게 찍거나 300dpi 로 다시 스캔해 주세요."
    } else {
        ""
    }
}

fn no_text_message(img: &image::DynamicImage) -> AppError {
    AppError::new(
        "OCR_NO_TEXT",
        format!(
            "사진에서 글자를 찾지 못했습니다.{} 품목을 직접 입력할 수 있습니다.",
            resolution_advice(img)
        ),
    )
}

fn no_table_message(img: &image::DynamicImage) -> AppError {
    AppError::new(
        "OCR_NO_TABLE",
        format!(
            "사진에서 견적서 표를 알아보지 못했습니다.{} 품목을 직접 입력할 수 있습니다.",
            resolution_advice(img)
        ),
    )
}

/// 이 PC 에서 한국어 OCR 을 쓸 수 있는가
pub fn available() -> bool {
    engine::korean_available()
}

/// 이미지 파일 하나를 읽어 표를 돌려준다.
pub fn read_image(path: &Path, origin: &str) -> AppResult<Vec<RawTable>> {
    let img = image::open(path).map_err(|e| {
        AppError::new("IMAGE_OPEN_FAILED", "그림 파일을 열지 못했습니다. 파일이 손상되었거나 지원하지 않는 형식일 수 있습니다. 품목을 직접 입력할 수 있습니다.").detail(e.to_string())
    })?;
    read_dynamic_image(&img, origin)
}

/// 이미 메모리에 있는 그림(스캔 PDF 에서 꺼낸 것 등)을 읽는다.
pub fn read_dynamic_image(img: &image::DynamicImage, origin: &str) -> AppResult<Vec<RawTable>> {
    let words = recognize(img)?;
    if words.is_empty() {
        return Err(no_text_message(img));
    }

    let blocks: Vec<Block> = words
        .into_iter()
        .map(|w| Block {
            text: w.text,
            x0: w.x,
            x1: w.x + w.width,
            y0: w.y,
            y1: w.y + w.height,
            origin: origin.to_string(),
        })
        .collect();

    // 표 제목은 위쪽 글자에서 (견적서 / 납품서 구분)
    let title = top_text(&blocks);
    let gap = guess_gap(&blocks);

    match layout::to_raw_table(blocks, &title, gap) {
        Some(t) => Ok(vec![t]),
        None => Err(no_table_message(img)),
    }
}

/// 인식한 **한 줄**과 그 상자. 견적서 표에서는 칸 하나가 한 줄로 온다.
pub struct TextBox {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// 3배로 키워 인식한 뒤, 좌표는 **원본 기준**으로 되돌린다.
pub fn recognize(img: &image::DynamicImage) -> AppResult<Vec<TextBox>> {
    let (w, h) = (img.width(), img.height());
    if w == 0 || h == 0 {
        return Ok(Vec::new());
    }
    // 10000px 을 넘지 않는 선에서 최대한 키운다 (기본 3배)
    let limit = (MAX_SIDE / w.max(1)).min(MAX_SIDE / h.max(1)).max(1);
    let scale = SCALE.min(limit.max(1));

    let big = if scale > 1 {
        image::DynamicImage::ImageRgba8(image::imageops::resize(
            &img.to_rgba8(),
            w * scale,
            h * scale,
            image::imageops::FilterType::Lanczos3,
        ))
    } else {
        img.clone()
    };

    let words = engine::recognize_korean(&big)?;
    let s = scale as f64;
    Ok(words
        .into_iter()
        .map(|mut x| {
            x.x /= s;
            x.y /= s;
            x.width /= s;
            x.height /= s;
            x
        })
        .collect())
}

fn top_text(blocks: &[Block]) -> String {
    let mut top: Vec<&Block> = blocks.iter().collect();
    top.sort_by(|a, b| a.y0.partial_cmp(&b.y0).unwrap_or(std::cmp::Ordering::Equal));
    top.iter().take(6).map(|b| b.text.as_str()).collect::<Vec<_>>().join(" ")
}

fn guess_gap(blocks: &[Block]) -> f64 {
    let mut sizes: Vec<f64> = blocks.iter().map(|b| b.height()).filter(|h| *h > 0.0).collect();
    if sizes.is_empty() {
        return 8.0;
    }
    sizes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    // 이미 줄 단위로 오므로 거의 붙일 것이 없다. 아주 가까운 것만 합친다.
    sizes[sizes.len() / 2] * 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img(w: u32, h: u32) -> image::DynamicImage {
        image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(w, h, image::Rgb([255; 3])))
    }

    #[test]
    fn real_sample_size_is_not_called_small() {
        // P0-6 에서 통과한 실제 주산암산 JPG 크기
        assert!(!looks_low_resolution(&img(794, 1123)));
        assert_eq!(resolution_advice(&img(794, 1123)), "");
    }

    #[test]
    fn tiny_photo_gets_advice() {
        assert!(looks_low_resolution(&img(640, 480)));
        assert!(resolution_advice(&img(640, 480)).contains("300dpi"));
    }

    /// 안내는 **막는 것이 아니다** — 작은 사진도 읽기는 시도한다
    #[test]
    fn advice_is_only_advice() {
        let small = img(300, 200);
        let e = no_table_message(&small);
        assert_eq!(e.code, "OCR_NO_TABLE");
        assert!(e.message.contains("직접 입력"), "{}", e.message);
        assert!(e.message.contains("사진이 작아"), "{}", e.message);
    }

    /// 실패 안내는 **다음에 무엇을 할지**를 반드시 말한다
    #[test]
    fn every_failure_says_what_to_do_next() {
        for m in [no_text_message(&img(2000, 3000)), no_table_message(&img(2000, 3000))] {
            assert!(m.message.contains("직접 입력"), "{}", m.message);
            // 기술 용어를 그대로 보여 주지 않는다
            assert!(!m.message.contains("OCR"), "{}", m.message);
            assert!(!m.message.contains("Err"), "{}", m.message);
        }
    }
}
