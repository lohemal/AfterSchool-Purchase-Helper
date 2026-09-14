//! 실제 업무 샘플로 견적서 파서를 확인한다.
//!
//! 샘플은 `test/fixtures-local/`(비커밋)에 있다. 없으면 조용히 건너뛴다.
//! HWP 는 한글이 있어야 하므로 `#[ignore]` 로 둔다:
//!   cargo test hwp_real -- --ignored --nocapture --test-threads=1

use super::*;
use crate::domain::RowKind;

fn fixture(name: &str) -> Option<std::path::PathBuf> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()?
        .join("test")
        .join("fixtures-local")
        .join(name);
    if !p.exists() {
        eprintln!("{}", need_local_fixture(name));
        return None;
    }
    Some(p)
}

/// 실제 업무 샘플이 없을 때 하는 말.
///
/// 샘플에는 거래처 개인정보와 학교 회계 자료가 들어 있어 **공개 저장소에 넣지 않는다**
/// (`test/fixtures-local/` 은 Git 이 무시한다). 그래서 내려받아 바로 돌리면 이 시험들은 건너뛴다.
/// 나머지 시험은 샘플 없이도 전부 돈다.
fn need_local_fixture(name: &str) -> String {
    format!(
        "건너뜀 — 이 시험에는 로컬 전용 샘플이 필요합니다: test/fixtures-local/{name}\n\
         (실제 업무 자료라 저장소에 넣지 않습니다. 자세한 내용은 docs/08-P5-배포준비.md)"
    )
}

#[test]
fn baduk_xlsx_quote() {
    let Some(path) = fixture("바둑 견적서.xlsx") else {
        return;
    };
    let q = parse(&path).expect("바둑 견적서 파싱");

    let items: Vec<_> = q.items.iter().filter(|i| i.kind == RowKind::Item).collect();
    assert_eq!(items.len(), 1, "품목 1건 (빈 행 17개는 섞이면 안 된다)");
    assert_eq!(items[0].display_name, "바둑교재(상상바둑)", "품목명은 원문 그대로");
    assert_eq!(items[0].raw_name, "바둑교재(상상바둑)");
    assert_eq!(items[0].spec, "권");
    assert_eq!(items[0].qty, Some(37));
    assert_eq!(items[0].unit_price, Some(12_000));
    assert_eq!(items[0].amount, Some(444_000));
    assert!(items[0].cell_ref.starts_with("견적서!"), "원본 셀 위치: {}", items[0].cell_ref);

    assert_eq!(q.item_sum, 444_000);
    assert_eq!(q.adjustment_sum, 0);
    assert_eq!(q.computed_total, 444_000);
    assert_eq!(q.grand_total, Some(444_000), "합계금액 칸");
    assert_eq!(q.compare_total(), 444_000);

    // 정산자료 바둑 = 372,000 + 12,000 + 60,000 + 0
    assert_eq!(q.compare_total(), 372_000 + 12_000 + 60_000);

    // 수량 x 단가 = 금액
    assert!(items[0].warnings.is_empty(), "{:?}", items[0].warnings);

    // 문구
    let rows: Vec<crate::domain::phrase::PhraseRow> = q
        .items
        .iter()
        .map(|i| crate::domain::phrase::PhraseRow {
            kind: i.kind,
            display_name: i.display_name.clone(),
        })
        .collect();
    assert_eq!(
        crate::domain::phrase::build("바둑부", &rows, None).unwrap(),
        "바둑부 바둑교재(상상바둑) 1종"
    );
}

#[test]
#[ignore = "한글이 깔린 PC 에서만 돈다"]
fn robot_hwp_real() {
    let Some(path) = fixture("로봇과학 견적서.hwp") else {
        return;
    };
    let before = std::fs::metadata(&path).unwrap().len();

    let q = parse(&path).expect("로봇과학 HWP 파싱");
    println!("표 고름: {}", q.table_note);
    assert!(q.table_note.contains("견적서"), "납품서가 아니라 견적서를 골라야 한다");

    let items: Vec<_> = q.items.iter().filter(|i| i.kind == RowKind::Item).collect();
    assert_eq!(items.len(), 2, "품목 2건");
    assert_eq!(items[0].display_name, "프로보테크닉 교구");
    assert_eq!(items[0].spec, "set");
    assert_eq!(items[0].qty, Some(10));
    assert_eq!(items[0].unit_price, Some(76_500));
    assert_eq!(items[0].amount, Some(765_000));
    assert_eq!(items[1].display_name, "프로보테크닉 교재");
    assert_eq!(items[1].amount, Some(135_000));

    assert_eq!(q.item_sum, 900_000);
    assert_eq!(q.compare_total(), 900_000, "합계금액 (￦ 900,000 ) 을 양수로 읽어야 한다");

    let rows: Vec<crate::domain::phrase::PhraseRow> = q
        .items
        .iter()
        .map(|i| crate::domain::phrase::PhraseRow {
            kind: i.kind,
            display_name: i.display_name.clone(),
        })
        .collect();
    assert_eq!(
        crate::domain::phrase::build("로봇과학부", &rows, None).unwrap(),
        "로봇과학부 프로보테크닉 교구 외 1종"
    );

    // 원본은 바뀌지 않아야 한다
    assert_eq!(std::fs::metadata(&path).unwrap().len(), before);
}

/// P3 — 주산암산 실제 JPG 를 **제품 경로**로 끝까지 읽는다 (탐침 코드가 아니다).
/// 기대값은 P0-6 에서 실측한 것과 같아야 한다.
#[test]
#[ignore = "Windows 한국어 OCR 이 있어야 돈다 (한 장에 20초쯤 걸린다)"]
fn jusan_jpg_ocr_real() {
    let Some(path) = fixture("주산암산 견적서.jpg") else {
        return;
    };
    let before = std::fs::metadata(&path).unwrap().len();

    let started = std::time::Instant::now();
    let q = parse(&path).expect("주산암산 JPG 읽기");
    println!("OCR {:.1}초 · {}", started.elapsed().as_secs_f64(), q.table_note);

    assert_eq!(q.source, "ocr");
    assert_eq!(q.trust, "needs_check", "사진은 사람이 한 번 봐야 한다");

    let items: Vec<_> = q.items.iter().filter(|i| i.kind == RowKind::Item).collect();
    for it in &items {
        println!("  {} / {} / {:?} x {:?} = {:?}", it.display_name, it.spec, it.qty, it.unit_price, it.amount);
    }

    assert_eq!(items.len(), 4, "품목 4건");
    assert_eq!(items[0].display_name, "방과후 기초Yap! 상");
    assert_eq!(items[0].qty, Some(3));
    assert_eq!(items[0].unit_price, Some(10_000));
    assert_eq!(items[0].amount, Some(30_000));
    assert_eq!(items[1].display_name, "방과후 기초Yap! 하");
    assert_eq!(items[1].amount, Some(10_000));
    assert_eq!(items[2].display_name, "10급Yap!");
    assert_eq!(items[2].qty, Some(2));
    assert_eq!(items[2].amount, Some(20_000));
    assert_eq!(items[3].display_name, "암산교재");
    assert_eq!(items[3].amount, Some(8_000));

    // 네 행 모두 수량×단가=금액 → 숫자를 제대로 읽었다는 근거
    for it in &items {
        assert!(
            !it.warnings.iter().any(|w| w.code == "QTY_PRICE_MISMATCH"),
            "{} {:?}",
            it.display_name,
            it.warnings
        );
        assert_eq!(it.confidence, crate::domain::Confidence::Medium, "사진은 high 를 주지 않는다");
    }

    assert_eq!(q.item_sum, 68_000, "품목 합");
    assert_eq!(q.compare_total(), 68_000);
    // 정산자료 주산암산: 40,000 + 0 + 18,000 + 10,000
    assert_eq!(q.compare_total(), 40_000 + 0 + 18_000 + 10_000);

    // 원본 위치
    assert_eq!(items[0].cell_ref, "사진");

    // 문구
    let rows: Vec<crate::domain::phrase::PhraseRow> = q
        .items
        .iter()
        .map(|i| crate::domain::phrase::PhraseRow {
            kind: i.kind,
            display_name: i.display_name.clone(),
        })
        .collect();
    assert_eq!(
        crate::domain::phrase::build("주산암산부", &rows, None).unwrap(),
        "주산암산부 방과후 기초Yap! 상 외 3종"
    );

    // 원본 그림은 바뀌지 않아야 한다
    assert_eq!(std::fs::metadata(&path).unwrap().len(), before);
}

/// 읽을 수 없는 형식은 **막지 말고 안내**해야 한다 (작업이 멈추면 안 된다)
#[test]
fn unsupported_formats_explain_instead_of_crashing() {
    let dir = std::env::temp_dir().join("quotemgr-test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("메모.txt");
    std::fs::write(&path, b"hello").unwrap();
    let e = parse(&path).unwrap_err();
    assert_eq!(e.code, "QUOTE_UNSUPPORTED");
    assert!(e.message.contains("직접 입력"), "{}", e.message);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn format_detection() {
    use crate::quote::model::QuoteFormat;
    use std::path::Path;
    assert_eq!(QuoteFormat::from_path(Path::new("a.xlsx")), QuoteFormat::Xlsx);
    assert_eq!(QuoteFormat::from_path(Path::new("a.XLSX")), QuoteFormat::Xlsx);
    assert_eq!(QuoteFormat::from_path(Path::new("a.hwp")), QuoteFormat::Hwp);
    assert_eq!(QuoteFormat::from_path(Path::new("a.hwpx")), QuoteFormat::Hwpx);
    assert_eq!(QuoteFormat::from_path(Path::new("a.jpg")), QuoteFormat::Image);
    assert_eq!(QuoteFormat::from_path(Path::new("a.pdf")), QuoteFormat::Pdf);
    assert!(QuoteFormat::Xlsx.supported());
    assert!(QuoteFormat::Hwp.supported());
    // P2·P3 에서 붙였다
    assert!(QuoteFormat::Image.supported());
    assert!(QuoteFormat::Pdf.supported());
    assert!(!QuoteFormat::Unsupported.supported());
    // 사진·PDF 는 오래 걸린다 → 화면이 멈춘 것처럼 보이면 안 된다
    assert!(QuoteFormat::Image.is_slow());
    assert!(QuoteFormat::Pdf.is_slow());
    assert!(!QuoteFormat::Xlsx.is_slow());
}

/// 진단용 — OCR 이 어떤 덩어리를 돌려주는지 눈으로 본다
#[test]
#[ignore = "진단용"]
fn dump_ocr_blocks() {
    let Some(path) = fixture("주산암산 견적서.jpg") else { return };
    let img = image::open(&path).unwrap();
    let words = crate::quote::ocr::recognize(&img).expect("OCR");
    println!("덩어리 {}개", words.len());
    let mut sorted: Vec<_> = words.iter().collect();
    sorted.sort_by(|a, b| a.y.partial_cmp(&b.y).unwrap());
    for w in sorted.iter().take(60) {
        println!("  {:>7.1},{:>7.1} {:>6.1}x{:>5.1}  {:?}", w.x, w.y, w.width, w.height, w.text);
    }
}

/// 시험 산출물 자리. **원본 샘플은 절대 건드리지 않는다.**
fn out_dir() -> std::path::PathBuf {
    let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("test").join("out");
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// P2 — **실물 텍스트 PDF**. 한글로 실제 견적서(HWP)를 PDF 로 내보내 제품 경로로 읽는다.
///
/// 텍스트 PDF 샘플을 따로 받지 못했으므로, 업무에서 실제로 쓰는 문서를
/// 업무에서 실제로 쓰는 프로그램(한글)이 만든 PDF 로 바꿔 시험한다.
/// 기대값은 같은 문서를 HWP 로 읽은 것과 **한 글자도 다르면 안 된다**.
#[test]
#[ignore = "한글이 깔린 PC 에서만 돈다"]
fn robot_pdf_text_real() {
    let Some(path) = fixture("로봇과학 견적서.hwp") else {
        return;
    };
    let before = std::fs::metadata(&path).unwrap().len();
    let pdf = out_dir().join("로봇과학 견적서.pdf");
    let _ = std::fs::remove_file(&pdf);
    crate::quote::hwp::save_as(&path, &pdf, "PDF").expect("한글로 PDF 내보내기");
    assert!(pdf.exists(), "PDF 가 만들어져야 한다");
    // 원본 HWP 는 그대로여야 한다
    assert_eq!(std::fs::metadata(&path).unwrap().len(), before);

    // **확장자가 아니라 내용으로** 텍스트 PDF 인지 가린다
    assert_eq!(crate::quote::pdf_text::probe(&pdf).unwrap(), crate::quote::pdf_text::PdfKind::Text);

    let q = parse(&pdf).expect("PDF 읽기");
    println!("{} · {}", q.source, q.table_note);
    assert_eq!(q.source, "pdf_text");

    let items: Vec<_> = q.items.iter().filter(|i| i.kind == RowKind::Item).collect();
    for it in &items {
        println!("  {} / {} / {:?} x {:?} = {:?} @ {}", it.display_name, it.spec, it.qty, it.unit_price, it.amount, it.cell_ref);
    }
    assert_eq!(items.len(), 2, "품목 2건 — HWP 로 읽은 것과 같아야 한다");
    assert_eq!(items[0].display_name, "프로보테크닉 교구");
    assert_eq!(items[0].spec, "set");
    assert_eq!(items[0].qty, Some(10));
    assert_eq!(items[0].unit_price, Some(76_500));
    assert_eq!(items[0].amount, Some(765_000));
    assert_eq!(items[1].display_name, "프로보테크닉 교재");
    assert_eq!(items[1].amount, Some(135_000));
    assert_eq!(q.item_sum, 900_000);

    // 원본 위치에 **쪽 번호**가 남아야 한다
    assert!(items[0].cell_ref.contains('쪽'), "원본 위치: {}", items[0].cell_ref);

    let rows: Vec<crate::domain::phrase::PhraseRow> = q
        .items
        .iter()
        .map(|i| crate::domain::phrase::PhraseRow { kind: i.kind, display_name: i.display_name.clone() })
        .collect();
    assert_eq!(
        crate::domain::phrase::build("로봇과학부", &rows, None).unwrap(),
        "로봇과학부 프로보테크닉 교구 외 1종"
    );
}

/// 실제 주산암산 JPG 를 통째로 품은 **스캔 PDF** 를 만든다 (시험용).
fn make_scan_pdf(jpg: &std::path::Path, dest: &std::path::Path) {
    use pdf_extract::{Dictionary, Document, Object, Stream};

    let bytes = std::fs::read(jpg).unwrap();
    let img = image::open(jpg).unwrap();
    let (w, h) = (img.width() as i64, img.height() as i64);

    let mut doc = Document::with_version("1.5");

    let mut d = Dictionary::new();
    d.set("Type", Object::Name(b"XObject".to_vec()));
    d.set("Subtype", Object::Name(b"Image".to_vec()));
    d.set("Width", w);
    d.set("Height", h);
    d.set("ColorSpace", Object::Name(b"DeviceRGB".to_vec()));
    d.set("BitsPerComponent", 8i64);
    d.set("Filter", Object::Name(b"DCTDecode".to_vec()));
    let img_id = doc.add_object(Stream::new(d, bytes));

    let content = format!("q {w} 0 0 {h} 0 0 cm /Im0 Do Q");
    let content_id = doc.add_object(Stream::new(Dictionary::new(), content.into_bytes()));

    let mut xobj = Dictionary::new();
    xobj.set("Im0", Object::Reference(img_id));
    let mut res = Dictionary::new();
    res.set("XObject", Object::Dictionary(xobj));

    let pages_id = doc.new_object_id();
    let mut page = Dictionary::new();
    page.set("Type", Object::Name(b"Page".to_vec()));
    page.set("Parent", Object::Reference(pages_id));
    page.set("Contents", Object::Reference(content_id));
    page.set("Resources", Object::Dictionary(res));
    page.set(
        "MediaBox",
        Object::Array(vec![0.into(), 0.into(), Object::Integer(w), Object::Integer(h)]),
    );
    let page_id = doc.add_object(page);

    let mut pages = Dictionary::new();
    pages.set("Type", Object::Name(b"Pages".to_vec()));
    pages.set("Kids", Object::Array(vec![Object::Reference(page_id)]));
    pages.set("Count", 1i64);
    doc.objects.insert(pages_id, Object::Dictionary(pages));

    let mut cat = Dictionary::new();
    cat.set("Type", Object::Name(b"Catalog".to_vec()));
    cat.set("Pages", Object::Reference(pages_id));
    let cat_id = doc.add_object(cat);
    doc.trailer.set("Root", Object::Reference(cat_id));

    doc.save(dest).unwrap();
}

/// P3 — **스캔 PDF**. 그림을 꺼내 사진 경로를 그대로 탄다. 쪽 번호가 원본 위치로 남아야 한다.
#[test]
#[ignore = "Windows 한국어 OCR 이 있어야 돈다"]
fn jusan_scan_pdf_real() {
    let Some(jpg) = fixture("주산암산 견적서.jpg") else {
        return;
    };
    let pdf = out_dir().join("주산암산 스캔.pdf");
    let _ = std::fs::remove_file(&pdf);
    make_scan_pdf(&jpg, &pdf);

    // 글자가 없는 PDF 이므로 **확장자가 아니라 내용으로** OCR 경로가 골라져야 한다
    match crate::quote::pdf_text::probe(&pdf).unwrap() {
        crate::quote::pdf_text::PdfKind::NeedsOcr(why) => println!("OCR 로 넘어간 까닭: {why}"),
        crate::quote::pdf_text::PdfKind::Text => panic!("스캔본을 텍스트 PDF 로 잘못 봤다"),
    }

    let q = parse(&pdf).expect("스캔 PDF 읽기");
    assert_eq!(q.source, "ocr");

    let items: Vec<_> = q.items.iter().filter(|i| i.kind == RowKind::Item).collect();
    assert_eq!(items.len(), 4, "JPG 로 읽은 것과 같아야 한다");
    assert_eq!(items[0].display_name, "방과후 기초Yap! 상");
    assert_eq!(items[3].display_name, "암산교재");
    assert_eq!(q.item_sum, 68_000);
    assert_eq!(q.compare_total(), 68_000);
    // 쪽 번호가 남는다
    assert_eq!(items[0].cell_ref, "1쪽");
}

/// 진단용 — 텍스트 PDF 에서 어떤 조각이 나오는지 눈으로 본다
#[test]
#[ignore = "진단용"]
fn dump_pdf_blocks() {
    let pdf = out_dir().join("로봇과학 견적서.pdf");
    if !pdf.exists() {
        eprintln!("PDF 없음 — robot_pdf_text_real 을 먼저 돌려라");
        return;
    }
    for t in crate::quote::pdf_text::read(&pdf).expect("표") {
        println!("### 표 {:?}", t.title);
        for r in 0..t.rows.len() {
            let cells: Vec<String> =
                (0..t.width()).map(|c| format!("{:?}", t.get(r, c))).collect();
            println!("    {}", cells.join(" | "));
        }
    }
    let pages = crate::quote::pdf_text::collect(&pdf).expect("PDF 글자");
    for (page, blocks) in pages {
        println!("--- {page}쪽 · 조각 {}개", blocks.len());
        let gap = crate::quote::pdf_text::guess_gap(&blocks);
        println!("    gap = {gap:.2}");
        let merged = crate::quote::layout::merge_adjacent(blocks, gap);
        for row in crate::quote::layout::group_rows(&merged) {
            let y = row[0].y0;
            let cells: Vec<String> = row.iter().map(|b| format!("{:?}@{:.0}", b.text, b.x0)).collect();
            println!("  y={y:>7.1}  {}", cells.join(" | "));
        }
    }
}

/// P4-3 — **읽기에 실패했을 때 사람에게 하는 말**을 한자리에서 확인한다.
///
/// 기술 오류 문자열을 그대로 보여 주지 않고, 언제나 **다음에 무엇을 할지**를 말해야 한다.
#[test]
fn failure_messages_tell_the_user_what_to_do() {
    let dir = std::env::temp_dir().join(format!("quotemgr-msg-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    // 형식별로 '깨진 파일' 을 만들어 실제 실패를 일으킨다
    let cases: Vec<(&str, &[u8])> = vec![
        ("깨진 견적서.xlsx", b"not a zip"),
        ("깨진 견적서.hwpx", b"not a zip"),
        ("깨진 견적서.pdf", b"%PDF-1.4 broken"),
        ("깨진 견적서.jpg", b"not an image"),
        ("메모.txt", b"hello"),
    ];

    for (name, body) in cases {
        let p = dir.join(name);
        std::fs::write(&p, body).unwrap();
        let e = parse(&p).expect_err(&format!("{name} 은 실패해야 한다"));
        println!("{name} → [{}] {}", e.code, e.message);

        // 1) 다음에 무엇을 할지 말한다
        assert!(
            e.message.contains("직접 입력"),
            "{name}: 다음에 할 일을 안 알려 준다 — {}",
            e.message
        );
        // 2) 기술 용어·원문 오류를 그대로 보여 주지 않는다
        for bad in ["Error", "error", "panic", "None", "Err(", "0x", "zip", "Invalid"] {
            assert!(!e.message.contains(bad), "{name}: 기술 용어 '{bad}' 가 보인다 — {}", e.message);
        }
        // 3) 자세한 원인은 detail 에만 둔다 (화면에는 message 만 나간다)
        assert!(!e.message.contains("detail"), "{}", e.message);
        let _ = std::fs::remove_file(&p);
    }

    let _ = std::fs::remove_dir_all(&dir);
}

/// 파일 내용 지문 (바이트가 하나라도 바뀌면 달라진다)
fn digest(path: &std::path::Path) -> String {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(path).expect("파일 읽기");
    format!("{:x}", Sha256::digest(&bytes))
}

/// P4-5 — **원본 견적서·정산자료는 읽기만 한다.** 바이트가 하나도 바뀌면 안 된다.
#[test]
fn originals_are_never_modified() {
    let Some(dir) = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|p| p.join("test").join("fixtures-local"))
        .filter(|p| p.is_dir())
    else {
        return;
    };

    let mut checked = 0;
    for entry in std::fs::read_dir(&dir).unwrap().flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let before = digest(&path);

        // 견적서로 읽어 본다 (실패해도 상관없다 — 원본을 건드렸는지가 관심사다)
        let _ = parse(&path);
        // 정산자료로도 읽어 본다
        let _ = crate::settlement::read(&path);
        // 원본 보기용 그림도 만들어 본다
        let _ = crate::quote::preview::build(&path);

        assert_eq!(before, digest(&path), "원본이 바뀌었다: {}", path.display());
        checked += 1;
    }
    assert!(checked > 0, "확인한 파일이 없다");
    println!("원본 {checked}개 — 모두 그대로");
}

/// 이 PC 에 떠 있는 `Hwp.exe` 개수
fn hwp_process_count() -> usize {
    let out = std::process::Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq Hwp.exe", "/NH"])
        .output();
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter(|l| l.to_lowercase().contains("hwp.exe"))
            .count(),
        Err(_) => 0,
    }
}

/// P4-5 — **한글을 부리고 나면 `Hwp.exe` 가 남으면 안 된다.**
///
/// 예전에는 `hangul_available()` 이 검사할 때마다 한글을 띄우고 끝내지 않아
/// 보이지 않는 프로세스가 쌓였다. 쌓이면 다음 변환이 느려지다가 결국 막힌다.
#[test]
#[ignore = "한글이 깔린 PC 에서만 돈다"]
fn hangul_leaves_no_process_behind() {
    let Some(path) = fixture("로봇과학 견적서.hwp") else {
        return;
    };
    let before = hwp_process_count();
    println!("시작 전 Hwp.exe {before}개");

    // 설치 여부 검사는 **한글을 띄우지 않는다**
    for _ in 0..5 {
        assert!(crate::quote::hwp::hangul_available());
    }
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert_eq!(hwp_process_count(), before, "설치 검사만으로 한글이 뜨면 안 된다");

    // 실제 변환을 두 번 해도 끝나면 남지 않는다
    for _ in 0..2 {
        parse(&path).expect("HWP 읽기");
    }
    std::thread::sleep(std::time::Duration::from_millis(1500));
    assert_eq!(hwp_process_count(), before, "변환이 끝나면 Hwp.exe 가 남지 않아야 한다");

    // 실패해도 남지 않는다 (되감기로 빠져나가는 길)
    let broken = std::env::temp_dir().join(format!("quotemgr-깨진-{}.hwp", std::process::id()));
    std::fs::write(&broken, b"not a hwp file").unwrap();
    let _ = parse(&broken);
    let _ = std::fs::remove_file(&broken);
    std::thread::sleep(std::time::Duration::from_millis(1500));
    assert_eq!(hwp_process_count(), before, "실패한 뒤에도 남지 않아야 한다");
}

/// 진단용 — 원본 보기가 실제로 얼마나 걸리고 얼마나 커지는지 잰다
#[test]
#[ignore = "진단용"]
fn measure_preview_cost() {
    let cases: Vec<(&str, std::path::PathBuf)> = vec![
        ("주산암산 JPG", fixture("주산암산 견적서.jpg").unwrap_or_default()),
        ("바둑 XLSX", fixture("바둑 견적서.xlsx").unwrap_or_default()),
        ("스캔 PDF", out_dir().join("주산암산 스캔.pdf")),
        ("글자 PDF", out_dir().join("로봇과학 견적서.pdf")),
    ];
    for (name, path) in cases {
        if !path.exists() {
            println!("{name}: 파일 없음 — 건너뜀");
            continue;
        }
        let file_kb = std::fs::metadata(&path).map(|m| m.len() / 1024).unwrap_or(0);
        let t = std::time::Instant::now();
        let r = crate::quote::preview::build(&path).expect("원본 보기");
        let ms = t.elapsed().as_millis();
        let b64: usize = r.pages.iter().map(|p| p.base64.len()).sum();
        println!(
            "{name}: 원본 {file_kb}KB → {}쪽 · base64 {}KB · {ms}ms · note={:?}",
            r.pages.len(),
            b64 / 1024,
            r.note
        );
    }

    // 휴대전화로 찍은 큰 사진 (4000x3000) 을 만들어 재 본다
    let big = std::env::temp_dir().join(format!("quotemgr-big-{}.jpg", std::process::id()));
    let mut img = image::RgbImage::new(4000, 3000);
    for (x, y, p) in img.enumerate_pixels_mut() {
        *p = image::Rgb([(x % 255) as u8, (y % 255) as u8, 200]);
    }
    image::DynamicImage::ImageRgb8(img).save(&big).unwrap();
    let kb = std::fs::metadata(&big).unwrap().len() / 1024;
    let t = std::time::Instant::now();
    let r = crate::quote::preview::build(&big).unwrap();
    println!(
        "큰 사진 4000x3000: 원본 {kb}KB → base64 {}KB · {}ms",
        r.pages[0].base64.len() / 1024,
        t.elapsed().as_millis()
    );
    // 어디서 시간이 가는지 쪼개 본다
    let t = std::time::Instant::now();
    let opened = image::open(&big).unwrap();
    println!("  여는 데 {}ms", t.elapsed().as_millis());

    let t = std::time::Instant::now();
    let a = opened.resize(1600, 1600, image::imageops::FilterType::Triangle);
    println!("  resize(Triangle) {}ms → {}x{}", t.elapsed().as_millis(), a.width(), a.height());

    let t = std::time::Instant::now();
    let b = opened.thumbnail(1600, 1600);
    println!("  thumbnail {}ms → {}x{}", t.elapsed().as_millis(), b.width(), b.height());

    let t = std::time::Instant::now();
    let mut buf = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 82)
        .encode_image(&b.to_rgb8())
        .unwrap();
    println!("  jpeg 인코딩 {}ms → {}KB", t.elapsed().as_millis(), buf.len() / 1024);

    let _ = std::fs::remove_file(&big);
}

/// P4 수정 — **실제 샘플 원본 보기.** 사진·스캔 PDF·글자 PDF·XLSX 를 한 번에 본다.
///
/// 요지: 어떤 형식이든 **그림이 나오거나, 왜 없는지 말하거나** 둘 중 하나로 끝난다.
/// 그리고 **OCR 을 다시 돌리지 않는다** — 그림만 꺼내 줄인다.
#[test]
#[ignore = "실제 샘플과 test/out 의 PDF 가 있어야 돈다"]
fn preview_of_real_samples() {
    let Some(jpg) = fixture("주산암산 견적서.jpg") else {
        return;
    };
    let xlsx = fixture("바둑 견적서.xlsx").unwrap();
    let scan = out_dir().join("주산암산 스캔.pdf");
    let text_pdf = out_dir().join("로봇과학 견적서.pdf");

    // --- 사진 ---
    let before = digest(&jpg);
    let t = std::time::Instant::now();
    let r = crate::quote::preview::build(&jpg).expect("사진 원본 보기");
    println!("사진: {}쪽 · {}ms", r.pages.len(), t.elapsed().as_millis());
    assert_eq!(r.pages.len(), 1);
    assert_eq!(r.pages[0].label, "사진", "품목의 원본 위치와 같은 말을 쓴다");
    assert_eq!(r.pages[0].mime, "image/jpeg");
    assert!(!r.pages[0].base64.is_empty());
    assert!(r.pages[0].width <= 1600 && r.pages[0].height <= 1600);
    assert_eq!(digest(&jpg), before, "원본 사진이 바뀌면 안 된다");

    // --- 스캔 PDF: 쪽 번호가 붙고, OCR 은 돌지 않는다 ---
    if scan.exists() {
        let t = std::time::Instant::now();
        let r = crate::quote::preview::build(&scan).expect("스캔 PDF 원본 보기");
        let ms = t.elapsed().as_millis();
        println!("스캔 PDF: {}쪽 · {ms}ms", r.pages.len());
        assert_eq!(r.pages.len(), 1);
        assert_eq!(r.pages[0].label, "1쪽");
        // OCR 은 한 장에 6초가 넘는다. 그보다 훨씬 빨라야 '다시 돌리지 않았다' 는 뜻이다.
        assert!(ms < 3_000, "OCR 을 다시 돌린 것으로 보인다: {ms}ms");
    }

    // --- 글자 PDF: 그림이 없고, 왜 없는지 말한다 ---
    if text_pdf.exists() {
        let r = crate::quote::preview::build(&text_pdf).expect("글자 PDF");
        assert!(r.pages.is_empty(), "글자 PDF 는 그림을 만들지 않는다");
        assert!(r.note.contains("원본 위치"), "{}", r.note);
        assert!(r.note.contains("열어"), "{}", r.note);
    }

    // --- XLSX: 마찬가지 ---
    let r = crate::quote::preview::build(&xlsx).expect("XLSX");
    assert!(r.pages.is_empty());
    assert!(!r.note.is_empty(), "왜 그림이 없는지 말해야 한다");

    // --- 원본 파일이 없어졌을 때 ---
    let gone = std::env::temp_dir().join("없어진 견적서.jpg");
    let _ = std::fs::remove_file(&gone);
    let r = crate::quote::preview::build(&gone).expect("없는 파일도 실패가 아니라 안내다");
    assert!(r.pages.is_empty());
    assert!(r.note.contains("찾을 수 없습니다"), "{}", r.note);
}

/// P4 수정 — **한글이 쓰는 자리는 언제나 `%TEMP%` 안이다.**
///
/// 바깥 폴더를 바로 주면 한글이 "파일 접근 승인" 을 물어보는데, 창을 감춘 자동화에서는
/// 그 물음이 보이지 않아 영영 멈춘다(실제로 19분을 매달렸다).
#[test]
#[ignore = "한글이 깔린 PC 에서만 돈다"]
fn save_as_outside_temp_does_not_hang() {
    let Some(src) = fixture("로봇과학 견적서.hwp") else {
        return;
    };
    // %TEMP% 바깥, 그것도 처음 만드는 깊은 폴더
    let deep = out_dir().join(format!("한글저장-{}", std::process::id())).join("안쪽");
    let dest = deep.join("내보낸 견적서.pdf");
    let _ = std::fs::remove_dir_all(deep.parent().unwrap());

    assert!(!crate::quote::hwp::is_inside_work_dir(&dest), "시험 전제: TEMP 바깥이어야 한다");

    let t = std::time::Instant::now();
    crate::quote::hwp::save_as(&src, &dest, "PDF").expect("PDF 내보내기");
    let secs = t.elapsed().as_secs_f64();
    println!("TEMP 바깥으로 PDF 내보내기: {secs:.1}초");

    assert!(dest.exists(), "원하는 자리에 파일이 있어야 한다");
    assert!(std::fs::metadata(&dest).unwrap().len() > 1000, "빈 파일이면 안 된다");
    // 승인 창에 걸리면 분 단위로 매달린다. 넉넉히 잡아도 1분을 넘기면 안 된다.
    assert!(secs < 60.0, "너무 오래 걸린다: {secs:.1}초");

    // 임시 파일을 남기지 않는다
    let leftovers = std::fs::read_dir(std::env::temp_dir().join("quotemgr-hwp"))
        .map(|d| d.flatten().filter(|e| e.file_name().to_string_lossy().starts_with("save-")).count())
        .unwrap_or(0);
    assert_eq!(leftovers, 0, "TEMP 에 중간 파일이 남았다");

    let _ = std::fs::remove_dir_all(deep.parent().unwrap());
}
