//! 앱 오류. 화면에 그대로 보여 줄 한국어 문장을 늘 함께 들고 다닌다.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct AppError {
    /// 기계가 보는 이름 (예: "DB_OPEN_FAILED")
    pub code: String,
    /// 사용자에게 보여 줄 문장
    pub message: String,
    /// 개발자용 상세. **경로·금액·품명을 넣지 않는다.**
    pub detail: Option<String>,
}

impl AppError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self { code: code.into(), message: message.into(), detail: None }
    }
    pub fn detail(mut self, d: impl Into<String>) -> Self {
        self.detail = Some(d.into());
        self
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)?;
        if let Some(d) = &self.detail {
            write!(f, " ({d})")?;
        }
        Ok(())
    }
}

impl std::error::Error for AppError {}

pub type AppResult<T> = Result<T, AppError>;

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::new("DB_ERROR", "자료를 읽고 쓰는 중 문제가 생겼습니다.").detail(e.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::new("IO_ERROR", "파일을 읽고 쓰는 중 문제가 생겼습니다.").detail(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::new("JSON_ERROR", "자료 형식이 올바르지 않습니다.").detail(e.to_string())
    }
}
