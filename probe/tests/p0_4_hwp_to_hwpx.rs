//! P0-4  HWP → HWPX 변환 (한글 COM, Rust)
//!
//! 시험 내용: Rust 의 `windows` 크레이트 IDispatch 로 한글 2018 을 몰아
//! HWP 견적서를 HWPX 로 저장할 수 있는가. (PowerShell 로는 이미 성공했다 — Rust 에서도 되는지 본다.)
//! 샘플: test/fixtures-local/로봇과학 견적서.hwp  (읽기만 한다)
//! 산출: test/out/p0-4/out.hwpx
//!
//! 한글이 깔려 있어야만 도는 시험이라 `#[ignore]` 로 둔다:
//!   cargo test --test p0_4_hwp_to_hwpx -- --ignored --nocapture --test-threads=1

use probe::com::{as_bool, v_bool, v_i32, v_str, ComApartment, Dispatch};
use probe::{fixture, out_dir};
use std::path::PathBuf;

const SAMPLE: &str = "로봇과학 견적서.hwp";

/// 작업 폴더는 **%TEMP% 안**에 둔다.
/// 문서 통합 도구에서 확인한 사실: 한글은 %TEMP% 바깥 파일을 열 때 "파일 접근 승인" 창을 띄우고 멈춘다.
fn work_dir() -> PathBuf {
    let d = std::env::temp_dir().join("quotemgr-p0-4");
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("작업 폴더");
    d
}

pub fn convert(src: &std::path::Path) -> Result<PathBuf, String> {
    let wd = work_dir();
    let in_path = wd.join("in.hwp");
    std::fs::copy(src, &in_path).map_err(|e| format!("복사 실패: {e}"))?;
    let out_path = wd.join("out.hwpx");

    let _apt = ComApartment::init_sta().map_err(|e| e.to_string())?;
    let hwp = Dispatch::create("HWPFrame.HwpObject").map_err(|e| e.to_string())?;

    // 보안 모듈 등록(있으면 승인 창을 없애 준다). 없어도 TEMP 경유라 진행된다.
    match hwp.call("RegisterModule", &[v_str("FilePathCheckDLL"), v_str("FilePathCheckerModule")]) {
        Ok(v) => println!("  RegisterModule → {}", as_bool(&v)),
        Err(e) => println!("  RegisterModule 실패(무시): {e}"),
    }

    // 창 감추기 — XHwpWindows.Item(0).Visible = false
    match hwp
        .get_object("XHwpWindows")
        .and_then(|ws| ws.call_object("Item", &[v_i32(0)]))
        .and_then(|w| w.put("Visible", v_bool(false)))
    {
        Ok(()) => println!("  창 감추기 성공"),
        Err(e) => println!("  창 감추기 실패(무시): {e}"),
    }

    let opened = hwp
        .call("Open", &[v_str(&in_path.to_string_lossy()), v_str("HWP"), v_str("forceopen:true")])
        .map_err(|e| format!("Open: {e}"))?;
    println!("  Open → {}", as_bool(&opened));
    if !as_bool(&opened) {
        return Err("Open 이 거짓을 돌려줬다".into());
    }

    let saved = hwp
        .call("SaveAs", &[v_str(&out_path.to_string_lossy()), v_str("HWPX"), v_str("")])
        .map_err(|e| format!("SaveAs: {e}"))?;
    println!("  SaveAs → {}", as_bool(&saved));

    // 정리: 문서 닫고 한글 끝내기
    if let Ok(docs) = hwp.get_object("XHwpDocuments") {
        let _ = docs.call("Close", &[v_bool(false)]);
    }
    let _ = hwp.call("Quit", &[]);

    if !out_path.exists() {
        return Err("결과 파일이 생기지 않았다".into());
    }
    Ok(out_path)
}

#[test]
#[ignore = "한글 2018 이 깔린 PC 에서만 돈다"]
fn p0_4_hwp_to_hwpx_real() {
    let src = fixture(SAMPLE);
    let before = std::fs::metadata(&src).expect("샘플").len();

    println!("변환 시작: {}", SAMPLE);
    let started = std::time::Instant::now();
    let out_path = convert(&src).expect("HWPX 변환");
    let took = started.elapsed();

    let size = std::fs::metadata(&out_path).expect("결과 파일").len();
    println!("결과: {} ({} 바이트, {:.1}초)", out_path.display(), size, took.as_secs_f64());

    assert!(size > 10_000, "HWPX 가 너무 작다: {size} 바이트");

    // zip 인지, 한글 문서 구조인지 확인
    let f = std::fs::File::open(&out_path).expect("열기");
    let mut zip = zip::ZipArchive::new(f).expect("HWPX 는 zip 이다");
    let names: Vec<String> = zip.file_names().map(|s| s.to_string()).collect();
    println!("HWPX 안: {:?}", names);
    assert!(names.iter().any(|n| n == "Contents/section0.xml"), "section0.xml 이 있어야 한다");
    assert!(names.iter().any(|n| n == "mimetype"), "mimetype 이 있어야 한다");
    {
        use std::io::Read;
        let mut m = zip.by_name("mimetype").unwrap();
        let mut s = String::new();
        m.read_to_string(&mut s).unwrap();
        assert_eq!(s.trim(), "application/hwp+zip", "mimetype");
    }

    // 결과를 test/out 으로 옮겨 P0-5 가 쓴다
    let keep = out_dir().join("p0-4-로봇과학.hwpx");
    std::fs::copy(&out_path, &keep).expect("결과 보관");
    println!("보관: {}", keep.display());

    // **원본은 절대 바뀌지 않아야 한다**
    let after = std::fs::metadata(&src).expect("샘플").len();
    assert_eq!(before, after, "원본 HWP 가 바뀌었다");
}
