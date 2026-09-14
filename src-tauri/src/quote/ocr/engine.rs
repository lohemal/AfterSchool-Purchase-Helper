//! Windows 내장 OCR 을 부르는 유일한 자리 (WinRT `Windows.Media.Ocr`).
//!
//! P0-6 에서 확인한 것:
//!   - 이 PC 에는 한국어(`ko`) 인식기가 있다. 없으면 안내만 하고 다른 기능은 그대로 쓴다.
//!   - `OcrWord` 는 **신뢰도를 주지 않는다.** `BoundingRect` 만 쓴다.
//!   - 그림은 `SoftwareBitmap` 으로 넘겨야 한다 → 메모리 그림을 PNG 로 만들어 넘긴다.

use windows::core::HSTRING;
use windows::Globalization::Language;
use windows::Graphics::Imaging::BitmapDecoder;
use windows::Media::Ocr::OcrEngine;
use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};

use crate::error::{AppError, AppResult};
use crate::quote::ocr::TextBox;

/// 한국어 인식기를 쓸 수 있는가
pub fn korean_available() -> bool {
    language_tags().iter().any(|t| t.starts_with("ko"))
}

pub fn language_tags() -> Vec<String> {
    let Ok(langs) = OcrEngine::AvailableRecognizerLanguages() else { return Vec::new() };
    langs
        .into_iter()
        .filter_map(|l| l.LanguageTag().ok().map(|t| t.to_string()))
        .collect()
}

fn win_err(what: &str, e: windows::core::Error) -> AppError {
    AppError::new("OCR_FAILED", "사진에서 글자를 읽는 중 문제가 생겼습니다.")
        .detail(format!("{what}: {e}"))
}

/// 그림 한 장을 한국어로 인식한다. 좌표는 넘긴 그림 기준이다.
pub fn recognize_korean(img: &image::DynamicImage) -> AppResult<Vec<TextBox>> {
    if !korean_available() {
        return Err(AppError::new(
            "OCR_NO_KOREAN",
            "이 컴퓨터에 한국어 글자 인식 기능이 없습니다. \
             설정 > 시간 및 언어 > 언어에서 한국어의 '광학 문자 인식'을 추가하거나, 품목을 직접 입력해 주세요.",
        ));
    }

    // 메모리 → PNG → WinRT 스트림
    let mut png: Vec<u8> = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| AppError::new("OCR_ENCODE", "그림을 변환하지 못했습니다.").detail(e.to_string()))?;

    let stream = InMemoryRandomAccessStream::new().map_err(|e| win_err("스트림", e))?;
    {
        let writer = DataWriter::CreateDataWriter(&stream.GetOutputStreamAt(0).map_err(|e| win_err("출력", e))?)
            .map_err(|e| win_err("쓰기", e))?;
        writer.WriteBytes(&png).map_err(|e| win_err("바이트", e))?;
        writer.StoreAsync().map_err(|e| win_err("저장", e))?.get().map_err(|e| win_err("저장 대기", e))?;
        writer.FlushAsync().map_err(|e| win_err("비우기", e))?.get().map_err(|e| win_err("비우기 대기", e))?;
        let _ = writer.DetachStream();
    }
    stream.Seek(0).map_err(|e| win_err("되감기", e))?;

    let decoder = BitmapDecoder::CreateAsync(&stream)
        .map_err(|e| win_err("디코더", e))?
        .get()
        .map_err(|e| win_err("디코더 대기", e))?;
    let bitmap = decoder
        .GetSoftwareBitmapAsync()
        .map_err(|e| win_err("비트맵", e))?
        .get()
        .map_err(|e| win_err("비트맵 대기", e))?;

    let ko = Language::CreateLanguage(&HSTRING::from("ko")).map_err(|e| win_err("언어", e))?;
    let engine = OcrEngine::TryCreateFromLanguage(&ko).map_err(|e| win_err("엔진", e))?;
    let result = engine
        .RecognizeAsync(&bitmap)
        .map_err(|e| win_err("인식", e))?
        .get()
        .map_err(|e| win_err("인식 대기", e))?;

    // **줄 단위로 받는다.** 낱말 단위로 받으면 품명(`방과후 기초Yap! 상`)과
    // 머리글(`품 명/규 격`)이 조각나서 열을 알아볼 수 없다. P0-6 에서 검증한 것도 줄 단위다.
    // 견적서 표에서는 OCR 이 칸 하나를 한 줄로 끊어 주므로 이것이 곧 칸이 된다.
    let mut out = Vec::new();
    for line in result.Lines().map_err(|e| win_err("줄", e))? {
        let text = line.Text().map_err(|e| win_err("글자", e))?.to_string();
        if text.trim().is_empty() {
            continue;
        }
        // 줄의 상자는 낱말 상자들을 합쳐 만든다 (줄 자체는 상자를 주지 않는다)
        let mut bounds: Option<(f64, f64, f64, f64)> = None;
        for w in line.Words().map_err(|e| win_err("낱말", e))? {
            let r = w.BoundingRect().map_err(|e| win_err("상자", e))?;
            let (x0, y0) = (r.X as f64, r.Y as f64);
            let (x1, y1) = (x0 + r.Width as f64, y0 + r.Height as f64);
            bounds = Some(match bounds {
                None => (x0, y0, x1, y1),
                Some((a, b, c, d)) => (a.min(x0), b.min(y0), c.max(x1), d.max(y1)),
            });
        }
        let Some((x0, y0, x1, y1)) = bounds else { continue };
        out.push(TextBox { text, x: x0, y: y0, width: x1 - x0, height: y1 - y0 });
    }
    Ok(out)
}
