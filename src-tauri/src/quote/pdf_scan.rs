//! P3 스캔 PDF — 쪽 안에 들어 있는 **그림을 꺼내** 이미지 OCR 경로를 그대로 쓴다.
//!
//! 렌더러(pdfium 등)를 넣지 않는다. 6MB 짜리 이진 파일을 학교 배포판에 넣지 않으려는 결정이며
//! 설계안 6장에 적혀 있다. 스캔본은 쪽마다 그림 한 장이 통째로 들어 있는 것이 보통이라
//! 그 그림을 꺼내는 것으로 충분하다.
//!
//! **스캔 PDF 전용 품목 파서를 만들지 않는다.** 그림을 꺼낸 뒤부터는 JPG 와 완전히 같은 길을 간다.

use std::path::Path;

use pdf_extract::{Document, Object};

use crate::error::{AppError, AppResult};
use crate::quote::model::RawTable;

/// 쪽에서 꺼낸 그림 한 장
pub struct PageImage {
    pub page: u32,
    pub image: image::DynamicImage,
}

/// 쪽마다 가장 큰 그림 하나씩 꺼낸다 (스캔본은 쪽 전체가 그림 한 장이다).
pub fn extract_page_images(path: &Path) -> AppResult<Vec<PageImage>> {
    let doc = Document::load(path).map_err(|e| {
        AppError::new("PDF_OPEN_FAILED", "PDF 를 열지 못했습니다. 파일이 손상되었거나 암호가 걸려 있을 수 있습니다. 품목을 직접 입력할 수 있습니다.").detail(e.to_string())
    })?;

    let mut out = Vec::new();
    for (page_no, page_id) in doc.get_pages() {
        let Ok(resources) = page_resources(&doc, page_id) else { continue };
        let mut best: Option<(u64, image::DynamicImage)> = None;

        for (_, obj) in resources {
            let Ok((_, target)) = doc.dereference(&obj) else { continue };
            let Ok(stream) = target.as_stream() else { continue };
            let dict = &stream.dict;
            if dict.get(b"Subtype").and_then(|o| o.as_name()).map(|n| n != b"Image").unwrap_or(true) {
                continue;
            }
            let Some(img) = decode_image(stream) else { continue };
            let area = img.width() as u64 * img.height() as u64;
            if best.as_ref().map(|(a, _)| area > *a).unwrap_or(true) {
                best = Some((area, img));
            }
        }

        if let Some((area, img)) = best {
            // 너무 작은 그림은 로고·도장이다. 쪽 전체 스캔이 아니다.
            if area >= 200_000 {
                out.push(PageImage { page: page_no, image: img });
            }
        }
    }
    Ok(out)
}

fn page_resources(doc: &Document, page_id: (u32, u16)) -> AppResult<Vec<(Vec<u8>, Object)>> {
    let page = doc
        .get_object(page_id)
        .and_then(|o| o.as_dict())
        .map_err(|e| AppError::new("PDF_PAGE", "PDF 의 한 쪽을 읽지 못했습니다. 품목을 직접 입력할 수 있습니다.").detail(e.to_string()))?;

    let res = page
        .get(b"Resources")
        .and_then(|o| doc.dereference(o).map(|(_, x)| x))
        .and_then(|o| o.as_dict())
        .map_err(|e| AppError::new("PDF_RES", "PDF 안을 읽지 못했습니다. 품목을 직접 입력할 수 있습니다.").detail(e.to_string()))?;

    let xobjects = res
        .get(b"XObject")
        .and_then(|o| doc.dereference(o).map(|(_, x)| x))
        .and_then(|o| o.as_dict())
        .map_err(|e| AppError::new("PDF_XOBJ", "PDF 안에서 그림 목록을 읽지 못했습니다. 품목을 직접 입력할 수 있습니다.").detail(e.to_string()))?;

    Ok(xobjects.iter().map(|(k, v)| (k.to_vec(), v.clone())).collect())
}

/// 스트림 하나를 그림으로. **다시 압축하지 않는다** — 원본 바이트를 그대로 읽는다.
fn decode_image(stream: &pdf_extract::Stream) -> Option<image::DynamicImage> {
    let filters = filter_names(stream);
    let last = filters.last().map(|s| s.as_str()).unwrap_or("");

    match last {
        // JPEG 는 바이트가 그대로 들어 있다
        "DCTDecode" | "JPXDecode" => image::load_from_memory(&stream.content).ok(),
        // 그 밖(FlateDecode 등)은 풀어서 생 픽셀을 조립한다
        _ => {
            let s = stream.clone();
            let data = s.decompressed_content().ok()?;
            raw_to_image(&s.dict, &data)
        }
    }
}

fn filter_names(stream: &pdf_extract::Stream) -> Vec<String> {
    let Some(f) = stream.dict.get(b"Filter").ok() else { return Vec::new() };
    match f {
        Object::Name(n) => vec![String::from_utf8_lossy(n).to_string()],
        Object::Array(a) => a
            .iter()
            .filter_map(|o| o.as_name().ok())
            .map(|n| String::from_utf8_lossy(n).to_string())
            .collect(),
        _ => Vec::new(),
    }
}

/// 압축을 푼 생 픽셀을 그림으로 (회색 8비트 · RGB 24비트 · 1비트 흑백만 다룬다)
fn raw_to_image(dict: &pdf_extract::Dictionary, data: &[u8]) -> Option<image::DynamicImage> {
    let w = dict.get(b"Width").ok()?.as_i64().ok()? as u32;
    let h = dict.get(b"Height").ok()?.as_i64().ok()? as u32;
    let bpc = dict.get(b"BitsPerComponent").ok().and_then(|o| o.as_i64().ok()).unwrap_or(8);
    if w == 0 || h == 0 {
        return None;
    }

    let components = data.len() as u64 / (w as u64 * h as u64).max(1);
    match (bpc, components) {
        (8, 1) => image::GrayImage::from_raw(w, h, data.to_vec()).map(image::DynamicImage::ImageLuma8),
        (8, 3) => image::RgbImage::from_raw(w, h, data.to_vec()).map(image::DynamicImage::ImageRgb8),
        (1, _) => {
            // 1비트 흑백 — 한 줄이 바이트 단위로 정렬돼 있다
            let row_bytes = w.div_ceil(8) as usize;
            if data.len() < row_bytes * h as usize {
                return None;
            }
            let mut buf = Vec::with_capacity((w * h) as usize);
            for y in 0..h as usize {
                for x in 0..w as usize {
                    let byte = data[y * row_bytes + x / 8];
                    let bit = (byte >> (7 - (x % 8))) & 1;
                    // PDF 1비트 그림은 0 이 검정이다
                    buf.push(if bit == 0 { 0u8 } else { 255u8 });
                }
            }
            image::GrayImage::from_raw(w, h, buf).map(image::DynamicImage::ImageLuma8)
        }
        _ => None,
    }
}

/// 스캔 PDF 를 읽어 표를 돌려준다. 쪽마다 OCR 한 번.
pub fn read(path: &Path) -> AppResult<Vec<RawTable>> {
    let pages = extract_page_images(path)?;
    if pages.is_empty() {
        return Err(AppError::new(
            "PDF_NO_IMAGE",
            "PDF 에서 글자도 그림도 찾지 못했습니다. 품목을 직접 입력해 주세요.",
        ));
    }

    let mut tables = Vec::new();
    let mut last_err: Option<AppError> = None;
    for p in pages {
        match crate::quote::ocr::read_dynamic_image(&p.image, &format!("{}쪽", p.page)) {
            Ok(mut t) => tables.append(&mut t),
            Err(e) => last_err = Some(e),
        }
    }

    if tables.is_empty() {
        return Err(last_err.unwrap_or_else(|| {
            AppError::new(
                "OCR_NO_TABLE",
                "자동 인식에 실패했습니다. 품목을 직접 입력할 수 있습니다.",
            )
        }));
    }
    Ok(tables)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_bit_image_rebuilds() {
        // 2x2, 1비트: 0b10 / 0b01 → 검정·흰색 무늬
        let mut dict = pdf_extract::Dictionary::new();
        dict.set(*b"Width", 2i64);
        dict.set(*b"Height", 2i64);
        dict.set(*b"BitsPerComponent", 1i64);
        let data = vec![0b1000_0000u8, 0b0100_0000u8];
        let img = raw_to_image(&dict, &data).expect("복원");
        assert_eq!((img.width(), img.height()), (2, 2));
        let g = img.to_luma8();
        assert_eq!(g.get_pixel(0, 0).0[0], 255);
        assert_eq!(g.get_pixel(1, 0).0[0], 0);
    }

    #[test]
    fn gray_image_rebuilds() {
        let mut dict = pdf_extract::Dictionary::new();
        dict.set(*b"Width", 2i64);
        dict.set(*b"Height", 2i64);
        dict.set(*b"BitsPerComponent", 8i64);
        let img = raw_to_image(&dict, &[0, 64, 128, 255]).expect("복원");
        assert_eq!(img.to_luma8().get_pixel(1, 1).0[0], 255);
    }

    #[test]
    fn bad_size_is_none() {
        let mut dict = pdf_extract::Dictionary::new();
        dict.set(*b"Width", 0i64);
        dict.set(*b"Height", 0i64);
        assert!(raw_to_image(&dict, &[]).is_none());
    }
}
