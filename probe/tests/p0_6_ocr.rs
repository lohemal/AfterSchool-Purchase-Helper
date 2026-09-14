//! P0-6  이미지 견적서 OCR (Windows 내장 OCR, Rust WinRT)
//!
//! 시험 내용: Rust 에서 `Windows.Media.Ocr` 를 한국어로 부르고, 3배 확대한 뒤
//! **본문 행의 x 구간 군집**으로 열을 만들어 표를 복원한다(머리글 위치로 나누지 않는다 — 설계안 6-3).
//! 샘플: test/fixtures-local/주산암산 견적서.jpg  (읽기만 한다)
//!
//!   cargo test --test p0_6_ocr -- --ignored --nocapture --test-threads=1

use probe::{fixture, out_dir};
use windows::core::HSTRING;
use windows::Globalization::Language;
use windows::Graphics::Imaging::BitmapDecoder;
use windows::Media::Ocr::OcrEngine;
use windows::Storage::{FileAccessMode, StorageFile};

#[derive(Debug, Clone)]
struct Block {
    text: String,
    x0: f64,
    x1: f64,
    y0: f64,
    y1: f64,
}

impl Block {
    fn yc(&self) -> f64 {
        (self.y0 + self.y1) / 2.0
    }
    fn xc(&self) -> f64 {
        (self.x0 + self.x1) / 2.0
    }
    fn h(&self) -> f64 {
        self.y1 - self.y0
    }
}

/// 이미지를 확대해 PNG 로 저장하고 그 경로를 준다.
/// 실측: 원본 크기(794x1123)에서는 한글이 거의 다 깨지고, **3배**면 품목명·숫자가 정확하다.
fn scale_to_png(src: &std::path::Path, factor: u32) -> std::path::PathBuf {
    let img = image::open(src).expect("이미지 열기");
    let (w, h) = (img.width() * factor, img.height() * factor);
    assert!(w <= 10_000 && h <= 10_000, "Windows OCR 은 한 변 10000픽셀까지다");
    let big = image::imageops::resize(&img.to_rgba8(), w, h, image::imageops::FilterType::Lanczos3);
    let out = out_dir().join(format!("p0-6-확대x{factor}.png"));
    big.save(&out).expect("PNG 저장");
    out
}

fn ocr_blocks(png: &std::path::Path) -> (Vec<Block>, f64) {
    let path = HSTRING::from(png.to_string_lossy().as_ref());
    let file = StorageFile::GetFileFromPathAsync(&path).expect("파일 열기").get().expect("대기");
    let stream = file.OpenAsync(FileAccessMode::Read).expect("스트림").get().expect("대기");
    let decoder = BitmapDecoder::CreateAsync(&stream).expect("디코더").get().expect("대기");
    let bitmap = decoder.GetSoftwareBitmapAsync().expect("비트맵").get().expect("대기");

    let langs = OcrEngine::AvailableRecognizerLanguages().expect("언어 목록");
    let tags: Vec<String> =
        langs.into_iter().map(|l| l.LanguageTag().unwrap_or_default().to_string()).collect();
    println!("설치된 OCR 언어: {tags:?}");
    assert!(tags.iter().any(|t| t.starts_with("ko")), "한국어 OCR 이 없다");

    let ko = Language::CreateLanguage(&HSTRING::from("ko")).expect("ko");
    let engine = OcrEngine::TryCreateFromLanguage(&ko).expect("엔진 만들기");

    let result = engine.RecognizeAsync(&bitmap).expect("인식").get().expect("대기");
    let angle = result.TextAngle().ok().and_then(|a| a.Value().ok()).unwrap_or(0.0);

    let mut blocks = Vec::new();
    for line in result.Lines().expect("줄") {
        let text = line.Text().unwrap_or_default().to_string();
        let words = line.Words().expect("낱말");
        let mut b: Option<Block> = None;
        for w in words {
            let r = w.BoundingRect().expect("상자");
            let (x0, y0) = (r.X as f64, r.Y as f64);
            let (x1, y1) = (x0 + r.Width as f64, y0 + r.Height as f64);
            b = Some(match b {
                None => Block { text: text.clone(), x0, y0, x1, y1 },
                Some(p) => Block {
                    text: p.text,
                    x0: p.x0.min(x0),
                    y0: p.y0.min(y0),
                    x1: p.x1.max(x1),
                    y1: p.y1.max(y1),
                },
            });
        }
        if let Some(b) = b {
            blocks.push(b);
        }
    }
    (blocks, angle)
}

fn role_of(h: &str) -> Option<&'static str> {
    let h: String = h.chars().filter(|c| !c.is_whitespace()).collect();
    match h.as_str() {
        "품명" | "품목" | "물품명" | "내역" | "품명/규격" => Some("name"),
        "규격" | "사양" | "단위" => Some("spec"),
        "수량" => Some("qty"),
        "단가" => Some("unit_price"),
        "금액" | "공급가액" | "공급가" => Some("amount"),
        "세액" | "부가세" => Some("tax"),
        "비고" | "적요" => Some("note"),
        "순번" | "번호" | "No" => Some("no"),
        _ => None,
    }
}

fn parse_number(s: &str) -> Option<i64> {
    let t = s.trim();
    let d: String = t.chars().filter(|c| c.is_ascii_digit()).collect();
    if d.is_empty() {
        return None;
    }
    d.parse().ok()
}

#[test]
#[ignore = "Windows 한국어 OCR 이 있어야 돈다"]
fn p0_6_image_quote_ocr() {
    let src = fixture("주산암산 견적서.jpg");
    let before = std::fs::metadata(&src).expect("샘플").len();

    // --- 1. 확대 배율에 따른 인식률 비교 (설계안 7장의 근거) ---
    for factor in [1u32, 3] {
        let png = scale_to_png(&src, factor);
        let (blocks, _) = ocr_blocks(&png);
        let joined: String = blocks.iter().map(|b| b.text.as_str()).collect::<Vec<_>>().join(" ");
        let hits = ["방과후 기초Yap! 상", "방과후 기초Yap! 하", "10급Yap!", "암산교재"]
            .iter()
            .filter(|w| joined.contains(**w))
            .count();
        println!("배율 x{factor}: 블록 {}개, 품목명 {hits}/4 정확", blocks.len());
        if factor == 1 {
            assert!(hits < 4, "원본 크기에서도 다 맞으면 확대 규칙을 다시 생각해야 한다");
        }
    }

    // --- 2. 3배로 본격 인식 ---
    let png = scale_to_png(&src, 3);
    let (blocks, angle) = ocr_blocks(&png);
    println!("기울기 {angle}도, 블록 {}개", blocks.len());

    // --- 3. y 로 행 묶기 ---
    let mut sorted = blocks.clone();
    sorted.sort_by(|a, b| a.yc().partial_cmp(&b.yc()).unwrap());
    let mut heights: Vec<f64> = sorted.iter().map(|b| b.h()).collect();
    heights.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let tol = heights[heights.len() / 2] * 0.6;

    let mut rows: Vec<Vec<Block>> = Vec::new();
    for b in sorted {
        match rows.last_mut() {
            Some(last) if (b.yc() - last.last().unwrap().yc()).abs() <= tol => last.push(b),
            _ => rows.push(vec![b]),
        }
    }
    for r in rows.iter_mut() {
        r.sort_by(|a, b| a.x0.partial_cmp(&b.x0).unwrap());
    }
    println!("행 {}개 (허용치 {tol:.1})", rows.len());

    // --- 4. 머리글 행 ---
    let hr = (0..rows.len())
        .max_by_key(|i| rows[*i].iter().filter(|b| role_of(&b.text).is_some()).count())
        .expect("행");
    let hits = rows[hr].iter().filter(|b| role_of(&b.text).is_some()).count();
    println!("머리글 행 {hr} (일치 {hits}): {:?}", rows[hr].iter().map(|b| &b.text).collect::<Vec<_>>());
    assert!(hits >= 5, "머리글 낱말이 5개 이상 맞아야 한다");

    // --- 5. 열 만들기: **본문 행의 x 구간 군집** (머리글 위치로 나누지 않는다) ---
    let body: Vec<&Vec<Block>> = rows.iter().skip(hr + 1).filter(|r| r.len() >= 3).collect();
    println!("본문 후보 행 {}개", body.len());
    let mut spans: Vec<(f64, f64)> = body.iter().flat_map(|r| r.iter().map(|b| (b.x0, b.x1))).collect();
    spans.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let mut clusters: Vec<(f64, f64)> = Vec::new();
    for (x0, x1) in spans {
        match clusters.last_mut() {
            Some(c) if x0 <= c.1 => c.1 = c.1.max(x1),
            _ => clusters.push((x0, x1)),
        }
    }
    println!("열 군집 {}개: {:?}", clusters.len(), clusters.iter().map(|(a, b)| (a.round(), b.round())).collect::<Vec<_>>());

    // --- 6. 각 군집에 가장 가까운 머리글 이름 붙이기 ---
    let names: Vec<String> = clusters
        .iter()
        .map(|(a, b)| {
            let c = (a + b) / 2.0;
            rows[hr]
                .iter()
                .min_by(|p, q| {
                    (p.xc() - c).abs().partial_cmp(&(q.xc() - c).abs()).unwrap()
                })
                .map(|h| h.text.chars().filter(|c| !c.is_whitespace()).collect())
                .unwrap_or_default()
        })
        .collect();
    println!("열 이름: {names:?}");

    let col_of = |role: &str| names.iter().position(|n| role_of(n) == Some(role));
    let (ni, qi, ui, ai) = (
        col_of("name").expect("품명 열"),
        col_of("qty").expect("수량 열"),
        col_of("unit_price").expect("단가 열"),
        col_of("amount").expect("금액 열"),
    );

    // --- 7. 표 복원 ---
    let which = |b: &Block| -> usize {
        let c = b.xc();
        clusters
            .iter()
            .enumerate()
            .min_by(|(_, p), (_, q)| {
                ((p.0 + p.1) / 2.0 - c).abs().partial_cmp(&(((q.0 + q.1) / 2.0 - c).abs())).unwrap()
            })
            .map(|(i, _)| i)
            .unwrap_or(0)
    };
    let mut items = Vec::new();
    for r in &body {
        let mut cells = vec![String::new(); clusters.len()];
        for b in r.iter() {
            let k = which(b);
            if !cells[k].is_empty() {
                cells[k].push(' ');
            }
            cells[k].push_str(&b.text);
        }
        let (name, q, u, a) = (
            cells[ni].trim().to_string(),
            parse_number(&cells[qi]),
            parse_number(&cells[ui]),
            parse_number(&cells[ai]),
        );
        if name.is_empty() || q.is_none() || u.is_none() || a.is_none() {
            continue;
        }
        items.push((name, q.unwrap(), u.unwrap(), a.unwrap()));
    }

    println!("--- 복원된 품목 ---");
    for (n, q, u, a) in &items {
        let ok = if q * u == *a { "OK" } else { "불일치" };
        println!("  {n:<22} {q} x {u:>7} = {a:>8}  {ok}");
    }

    // --- 통과 기준 ---
    assert_eq!(items.len(), 4, "품목 4건");
    assert_eq!(items[0], ("방과후 기초Yap! 상".into(), 3, 10_000, 30_000));
    assert_eq!(items[1], ("방과후 기초Yap! 하".into(), 1, 10_000, 10_000));
    assert_eq!(items[2], ("10급Yap!".into(), 2, 10_000, 20_000));
    assert_eq!(items[3], ("암산교재".into(), 1, 8_000, 8_000));
    for (n, q, u, a) in &items {
        assert_eq!(q * u, *a, "{n} 수량x단가=금액");
    }
    let sum: i64 = items.iter().map(|(_, _, _, a)| a).sum();
    assert_eq!(sum, 68_000, "품목 합");
    assert_eq!(sum, 40_000 + 0 + 18_000 + 10_000, "정산자료 주산암산 재원 합계와 일치");

    // --- 8. 표기 합계는 깨지는 것이 정상 (설계안 8-2 근거) ---
    let stated: Option<i64> = blocks
        .iter()
        .find(|b| b.text.contains('₩') || b.text.contains('￦'))
        .and_then(|b| parse_number(&b.text));
    println!("표기 합계금액 읽기: {stated:?} (원본은 ₩68,000 — 깨지면 품목 합으로 대체한다)");
    assert_ne!(stated, Some(68_000), "이 샘플에서는 표기 합계가 깨지는 것이 현재 상태다");

    // --- 9. 문구 생성 ---
    let phrase = format!("주산암산부 {} 외 {}종", items[0].0, items.len() - 1);
    assert_eq!(phrase, "주산암산부 방과후 기초Yap! 상 외 3종", "사람이 실제로 쓴 문구와 같아야 한다");
    println!("자동 문구: {phrase}");

    let after = std::fs::metadata(&src).expect("샘플").len();
    assert_eq!(before, after, "원본 JPG 가 바뀌었다");
}
