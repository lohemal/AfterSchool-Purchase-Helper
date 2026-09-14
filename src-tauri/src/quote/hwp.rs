//! HWP 견적서 → 한글 COM 으로 HWPX 변환 → `hwpx.rs` (P0-4 에서 검증한 방식).
//!
//! 알아 둘 것 (문서 통합 도구에서 얻은 것 + P0-4 실측)
//!   - 한글은 **`%TEMP%` 바깥 파일을 열면 "파일 접근 승인" 창을 띄우고 멈춘다** → 작업 폴더를 TEMP 에 둔다.
//!   - `Open`·`SaveAs` 는 참을 **VT_BOOL 1** 로 돌려준다(-1 이 아니다) → `com::as_bool` 로 읽는다.
//!   - 끝나면 문서를 닫고 `Quit` 한다. 남은 `hwp.exe` 가 다음 실행을 방해한다.

use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};
use crate::quote::com::{as_bool, v_bool, v_i32, v_str, ComApartment, Dispatch};
use crate::quote::model::RawTable;

/// 한글이 이 PC 에 있는가 (없으면 HWP 를 못 읽는다)
///
/// **한글을 띄우지 않고** 등록부만 본다. 예전에는 여기서 객체를 만들었는데,
/// 그때마다 보이지 않는 `Hwp.exe` 가 하나씩 남아 쌓였다(P4-5).
pub fn hangul_available() -> bool {
    crate::quote::com::prog_id_installed(PROG_ID)
}

const PROG_ID: &str = "HWPFrame.HwpObject";

/// 한글 자동화 객체를 잡고 있다가 **버려질 때 반드시 끝낸다.**
///
/// 중간에 오류가 나거나 시험이 실패해 되감기(unwind)로 빠져나가도
/// `Drop` 이 돌기 때문에 `Hwp.exe` 가 남지 않는다.
struct HwpApp {
    d: Dispatch,
}

impl HwpApp {
    fn start() -> AppResult<Self> {
        let d = Dispatch::create(PROG_ID).map_err(|e| {
            AppError::new(
                "HWP_NOT_INSTALLED",
                "한글(HWP) 프로그램을 찾지 못했습니다. HWP 견적서를 읽으려면 한글이 설치돼 있어야 합니다.",
            )
            .detail(e.to_string())
        })?;
        Ok(Self { d })
    }
}

impl Drop for HwpApp {
    fn drop(&mut self) {
        // 자식 객체를 먼저 놓아 준 뒤에 끝내야 프로세스가 실제로 내려간다
        if let Ok(docs) = self.d.get_object("XHwpDocuments") {
            let _ = docs.call("Close", &[v_bool(false)]);
        }
        let _ = self.d.call("Quit", &[]);
    }
}

fn work_dir() -> AppResult<PathBuf> {
    let d = std::env::temp_dir().join("purchase-helper-hwp");
    std::fs::create_dir_all(&d)?;
    Ok(d)
}

/// 겹치지 않는 임시 이름 하나
fn stamp() -> String {
    // 같은 밀리초에 두 번 불려도 겹치지 않게 pid 와 일련번호를 함께 쓴다
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    format!(
        "{}-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed),
        chrono::Local::now().format("%H%M%S%3f")
    )
}

/// HWP → HWPX. 돌려주는 경로는 `%TEMP%` 안에 있고 호출한 쪽이 지워도 된다.
pub fn convert_to_hwpx(src: &Path) -> AppResult<PathBuf> {
    let out_path = work_dir()?.join(format!("out-{}.hwpx", stamp()));
    save_as(src, &out_path, "HWPX")?;
    Ok(out_path)
}

/// 한글이 쓰는 작업 폴더 안인가 (여기서만 승인 창 없이 저장된다)
pub fn is_inside_work_dir(p: &Path) -> bool {
    work_dir().map(|d| p.starts_with(d)).unwrap_or(false)
}

/// 한글로 문서를 다른 형식으로 저장한다 (`HWPX` · `PDF` …).
///
/// 원본은 건드리지 않는다.
///
/// **읽는 쪽도 쓰는 쪽도 반드시 `%TEMP%` 안에서 한다.**
/// 한글은 자기 작업 폴더 밖의 경로를 만나면 "파일 접근 승인" 을 물어보는데,
/// 창을 감춰 둔 자동화에서는 그 물음이 보이지 않아 **영영 멈춘다**
/// (실제로 바탕화면 밑 폴더로 PDF 를 내보내다 19분을 매달렸다).
/// 그래서 `%TEMP%` 에 만든 뒤 원하는 자리로 옮긴다.
pub fn save_as(src: &Path, out_path: &Path, format: &str) -> AppResult<()> {
    let wd = work_dir()?;
    let stamp = stamp();
    let in_path = wd.join(format!("in-{stamp}.hwp"));
    // 한글이 쓸 자리는 언제나 TEMP 안이다
    let ext = out_path.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default();
    let temp_out = wd.join(if ext.is_empty() {
        format!("save-{stamp}")
    } else {
        format!("save-{stamp}.{ext}")
    });

    // TEMP 로 복사해 승인 창을 피한다
    std::fs::copy(src, &in_path).map_err(|e| {
        AppError::new("HWP_COPY_FAILED", "한글 문서를 작업 폴더로 옮기지 못했습니다. 디스크 공간이나 파일 권한을 확인해 주세요.")
            .detail(e.to_string())
    })?;

    let result = (|| -> AppResult<()> {
        let _apt = ComApartment::init_sta().map_err(com_to_app)?;
        // 여기서부터는 무슨 일이 있어도 `app` 이 버려질 때 한글이 닫힌다
        let app = HwpApp::start()?;
        let hwp = &app.d;

        // 보안 모듈이 등록돼 있으면 승인 창이 아예 안 뜬다. 없어도 TEMP 경유라 진행된다.
        let _ = hwp.call(
            "RegisterModule",
            &[v_str("FilePathCheckDLL"), v_str("FilePathCheckerModule")],
        );

        // 창 감추기 (실패해도 진행)
        let _ = hwp
            .get_object("XHwpWindows")
            .and_then(|ws| ws.call_object("Item", &[v_i32(0)]))
            .and_then(|w| w.put("Visible", v_bool(false)));

        let opened = hwp
            .call(
                "Open",
                &[v_str(&in_path.to_string_lossy()), v_str("HWP"), v_str("forceopen:true")],
            )
            .map_err(com_to_app)?;
        if !as_bool(&opened) {
            return Err(AppError::new(
                "HWP_OPEN_FAILED",
                "한글 문서를 열지 못했습니다. 파일이 손상되었거나 암호가 걸려 있을 수 있습니다.",
            ));
        }

        let saved = hwp
            .call("SaveAs", &[v_str(&temp_out.to_string_lossy()), v_str(format), v_str("")])
            .map_err(com_to_app)?;
        if !as_bool(&saved) || !temp_out.exists() {
            return Err(AppError::new(
                "HWP_CONVERT_FAILED",
                "한글 문서를 변환하지 못했습니다. 한글 프로그램을 닫고 다시 해 보시거나, 품목을 직접 입력해 주세요.",
            ));
        }
        Ok(())
    })();

    let _ = std::fs::remove_file(&in_path);
    result?;

    // TEMP 에서 원하는 자리로 옮긴다 (드라이브가 다르면 rename 이 안 되므로 복사로 대체)
    if let Some(dir) = out_path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    if std::fs::rename(&temp_out, out_path).is_err() {
        std::fs::copy(&temp_out, out_path).map_err(|e| {
            AppError::new("HWP_MOVE_FAILED", "변환한 파일을 옮기지 못했습니다. 저장 위치를 확인해 주세요.")
                .detail(e.to_string())
        })?;
        let _ = std::fs::remove_file(&temp_out);
    }
    Ok(())
}

/// HWP 를 읽어 표를 돌려준다.
pub fn read(path: &Path) -> AppResult<Vec<RawTable>> {
    let hwpx = convert_to_hwpx(path)?;
    let tables = crate::quote::hwpx::read(&hwpx);
    let _ = std::fs::remove_file(&hwpx);
    tables
}

fn com_to_app(e: crate::quote::com::ComError) -> AppError {
    AppError::new("HWP_COM_FAILED", "한글 프로그램을 다루는 중 문제가 생겼습니다. 한글을 닫고 다시 해 보시거나, 품목을 직접 입력해 주세요.").detail(e.to_string())
}
