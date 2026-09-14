//! P4-2 원본 확인 — 사진·스캔 PDF 의 **원본 그림**을 화면에 그대로 보여 준다.
//!
//! OCR 로 읽은 값이 맞는지 보려고 매번 탐색기에서 파일을 다시 찾는 일을 없애려는 것이다.
//!
//! **새 의존성을 넣지 않는다.** 이미 쓰는 것만으로 되는 범위까지만 한다.
//!   - 사진(JPG·PNG) → 파일을 그대로 읽어 보여 준다
//!   - 스캔 PDF → 이미 P3 에서 쪽마다 꺼내고 있는 그림을 그대로 쓴다
//!   - 글자 PDF · XLSX · HWP → 그림이 없다. 쪽 번호만 알려 주고 **원본 파일 열기**로 넘긴다
//!     (PDF 를 그리려면 큰 렌더러가 필요하다. 임의로 넣지 않는다)

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::AppResult;
use crate::quote::model::QuoteFormat;

/// 화면에 보낼 수 있는 크기의 한 변 최대 픽셀.
/// 원본 그대로 보내면 사진 한 장이 수 MB 가 되어 화면이 버벅인다.
const MAX_SIDE: u32 = 1600;
/// JPEG 로 줄일 때 품질 (글자를 읽을 수 있어야 한다)
const QUALITY: u8 = 82;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewPage {
    /// 사람이 읽는 자리 이름 — 품목의 `원본 위치` 와 같은 말을 쓴다 (`사진` · `1쪽`)
    pub label: String,
    /// `data:` URL 에 그대로 넣을 수 있는 형태
    pub mime: String,
    pub base64: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotePreview {
    pub pages: Vec<PreviewPage>,
    /// 그림을 못 만들 때 사람에게 할 말 (빈 문자열이면 할 말 없음)
    pub note: String,
}

impl QuotePreview {
    fn note(msg: &str) -> Self {
        Self { pages: Vec::new(), note: msg.to_string() }
    }
}

/// 견적서 원본을 화면에서 볼 수 있는 그림으로. **원본 파일은 읽기만 한다.**
pub fn build(path: &Path) -> AppResult<QuotePreview> {
    if !path.exists() {
        return Ok(QuotePreview::note("원본 파일을 찾을 수 없습니다. 옮기거나 지우셨나요?"));
    }

    match QuoteFormat::from_path(path) {
        QuoteFormat::Image => {
            let img = image::open(path).map_err(|e| {
                crate::error::AppError::new("IMAGE_OPEN_FAILED", "그림 파일을 열지 못했습니다. 파일이 손상되었거나 지원하지 않는 형식일 수 있습니다. 품목을 직접 입력할 수 있습니다.")
                    .detail(e.to_string())
            })?;
            Ok(QuotePreview { pages: vec![to_page("사진", &img)?], note: String::new() })
        }

        QuoteFormat::Pdf => {
            // 스캔본이면 P3 이 이미 꺼내 쓰는 그림이 있다
            let images = crate::quote::pdf_scan::extract_page_images(path)?;
            if images.is_empty() {
                return Ok(QuotePreview::note(
                    "글자로 된 PDF 라 미리 보여 줄 그림이 없습니다. \
                     품목의 원본 위치에 적힌 쪽을 보시고, 아래 단추로 원본을 열어 확인해 주세요.",
                ));
            }
            let mut pages = Vec::new();
            for p in images {
                pages.push(to_page(&format!("{}쪽", p.page), &p.image)?);
            }
            Ok(QuotePreview { pages, note: String::new() })
        }

        _ => Ok(QuotePreview::note(
            "이 형식은 미리 보여 줄 그림이 없습니다. \
             품목의 원본 위치에 적힌 칸을 보시고, 아래 단추로 원본을 열어 확인해 주세요.",
        )),
    }
}

/// 화면에 보낼 크기로 줄여 JPEG 로 만든다. **원본 파일은 바뀌지 않는다.**
///
/// 많이 줄일 때는 `thumbnail`(상자 평균)을 쓴다. 휴대전화로 찍은 4000×3000 사진에서
/// `Triangle` 보다 **두 배 이상 빠르고**, 여러 픽셀을 평균 내므로 글자도 또렷하다.
/// 조금만 줄일 때는 `Triangle` 이 더 곱다.
fn to_page(label: &str, img: &image::DynamicImage) -> AppResult<PreviewPage> {
    let (w, h) = (img.width(), img.height());
    let long = w.max(h);
    let shrunk = if long > MAX_SIDE * 2 {
        img.thumbnail(MAX_SIDE, MAX_SIDE)
    } else if long > MAX_SIDE {
        img.resize(MAX_SIDE, MAX_SIDE, image::imageops::FilterType::Triangle)
    } else {
        img.clone()
    };

    let mut buf: Vec<u8> = Vec::new();
    let rgb = shrunk.to_rgb8();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, QUALITY)
        .encode_image(&rgb)
        .map_err(|e| {
            crate::error::AppError::new("PREVIEW_ENCODE", "원본 그림을 준비하지 못했습니다.")
                .detail(e.to_string())
        })?;

    Ok(PreviewPage {
        label: label.to_string(),
        mime: "image/jpeg".into(),
        base64: base64(&buf),
        width: shrunk.width(),
        height: shrunk.height(),
    })
}

/// 작은 base64 인코더 — 이것 하나 때문에 크레이트를 더 넣지 않는다.
fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let b = [c[0], *c.get(1).unwrap_or(&0), *c.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_the_standard() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
        assert_eq!(base64(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        // 0xFF 처럼 부호 있는 바이트도 맞아야 한다
        assert_eq!(base64(&[0xff, 0xff, 0xff]), "////");
        assert_eq!(base64(&[0x00, 0x00, 0x00]), "AAAA");
    }

    #[test]
    fn missing_file_explains_instead_of_failing() {
        let p = std::path::Path::new("없는파일.jpg");
        let r = build(p).unwrap();
        assert!(r.pages.is_empty());
        assert!(r.note.contains("찾을 수 없습니다"), "{}", r.note);
    }

    #[test]
    fn structured_formats_have_no_picture_but_say_so() {
        let dir = std::env::temp_dir().join("quotemgr-preview");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("견적서.xlsx");
        std::fs::write(&p, b"x").unwrap();
        let r = build(&p).unwrap();
        assert!(r.pages.is_empty());
        assert!(r.note.contains("원본 위치"), "{}", r.note);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn photo_is_shrunk_for_the_screen() {
        let img = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            3000,
            2000,
            image::Rgb([200, 200, 200]),
        ));
        let page = to_page("사진", &img).unwrap();
        assert_eq!(page.label, "사진");
        assert_eq!(page.mime, "image/jpeg");
        assert!(page.width <= MAX_SIDE && page.height <= MAX_SIDE, "{}x{}", page.width, page.height);
        assert!(!page.base64.is_empty());
        // 비율을 지킨다 (반올림 때문에 1픽셀까지는 어긋날 수 있다)
        let ratio = page.width as f64 / page.height as f64;
        assert!((ratio - 1.5).abs() < 0.01, "{}x{}", page.width, page.height);
    }

    /// 깨진 그림 파일은 **안내로 끝난다** (프로그램이 멈추거나 매달리지 않는다)
    #[test]
    fn broken_image_fails_with_a_clear_message() {
        let dir = std::env::temp_dir().join("quotemgr-preview");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(format!("깨진-{}.jpg", std::process::id()));
        std::fs::write(&p, b"not an image at all").unwrap();

        let e = build(&p).expect_err("깨진 그림은 실패해야 한다");
        assert_eq!(e.code, "IMAGE_OPEN_FAILED");
        assert!(e.message.contains("직접 입력"), "{}", e.message);
        let _ = std::fs::remove_file(&p);
    }

    /// 큰 사진도 화면 크기로 줄여 **빠르게** 돌려준다
    #[test]
    fn large_photo_is_shrunk_and_quick() {
        let img = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            4000,
            3000,
            image::Rgb([180, 180, 180]),
        ));
        let t = std::time::Instant::now();
        let page = to_page("사진", &img).unwrap();
        let ms = t.elapsed().as_millis();
        assert!(page.width <= MAX_SIDE && page.height <= MAX_SIDE);
        // 디버그 빌드는 느리므로 넉넉히 잡는다. 화면 시간 제한(20초)보다는 훨씬 짧아야 한다.
        assert!(ms < 10_000, "너무 오래 걸린다: {ms}ms");
        // 보내는 양도 감당할 만해야 한다
        assert!(page.base64.len() < 3_000_000, "{}바이트", page.base64.len());
    }

    /// 같은 파일을 여러 번 불러도 같은 결과다 (여러 번 눌러도 안전)
    #[test]
    fn repeated_calls_are_stable() {
        let dir = std::env::temp_dir().join("quotemgr-preview");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(format!("반복-{}.png", std::process::id()));
        image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(300, 200, image::Rgb([9; 3])))
            .save(&p)
            .unwrap();

        let a = build(&p).unwrap();
        let b = build(&p).unwrap();
        let c = build(&p).unwrap();
        assert_eq!(a.pages[0].base64, b.pages[0].base64);
        assert_eq!(b.pages[0].base64, c.pages[0].base64);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn small_photo_is_not_enlarged() {
        let img = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            100,
            50,
            image::Rgb([10, 10, 10]),
        ));
        let page = to_page("사진", &img).unwrap();
        assert_eq!((page.width, page.height), (100, 50));
    }
}
