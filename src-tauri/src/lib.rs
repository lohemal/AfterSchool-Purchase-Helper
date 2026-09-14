//! 방과후 품의 도우미 — 방과후 교재비·재료비 품의 자동 작성.
//!
//! 기준 문서는 `docs/01-설계안.md`. 용어(품의 부서 · 정산 별칭 · 거래처 관리명)는 그 문서 2장을 따른다.

pub mod commands;
pub mod datadir;
pub mod db;
pub mod domain;
pub mod edufine;
pub mod error;
pub mod logpolicy;
pub mod quote;
pub mod settlement;
pub mod tempclean;

use std::path::PathBuf;

use db::Db;

pub struct AppState {
    pub db: Db,
}

/// 자료를 두는 자리 — **프로그램이 설치된 곳이 아니라 사용자 앱 데이터 폴더**다.
/// (`%APPDATA%\kr.school.afterschool-purchase-helper`)
/// 그래야 새 버전으로 올려도, 지웠다 다시 깔아도 작업이 그대로 남는다.
fn data_dir(app: &tauri::AppHandle) -> PathBuf {
    use tauri::Manager;
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("kr.school.afterschool-purchase-helper"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let dir = data_dir(app.handle());
            let path = dir.join(datadir::DB_FILE);

            // 개발판(예전 이름)으로 해 둔 작업이 있으면 처음 한 번 옮겨 온다.
            // 예전 자료는 지우지 않는다.
            if let Some(root) = dir.parent() {
                if datadir::adopt_legacy(&path, &datadir::legacy_dbs(root)).is_some() {
                    // 어디서 가져왔는지는 로그에 적지 않는다 — 경로는 남기지 않는 것이 정책이다
                    log::info!("예전 자리의 자료를 옮겨 왔습니다");
                }
            }

            let db = Db::open(&path).map_err(|e| std::io::Error::other(e.to_string()))?;
            log::info!("자료 파일 준비 완료 (스키마 v{})", db.schema_version().unwrap_or(0));
            // 지난번에 갑자기 꺼져 남은 임시 파일을 치운다 (사용자 자료는 건드리지 않는다)
            let swept = tempclean::sweep();
            if swept > 0 {
                log::info!("남아 있던 임시 파일 {swept}개를 치웠습니다");
            }
            tauri::Manager::manage(app, AppState { db });
            Ok(())
        })
        .invoke_handler(commands::handler())
        .run(tauri::generate_context!())
        .expect("프로그램을 시작하지 못했습니다");
}
