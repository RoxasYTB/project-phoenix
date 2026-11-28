use std::time::Duration;

#[derive(Debug, Clone, serde::Deserialize, Default)]
pub struct ApiErrorBody {
    #[serde(default)]
    pub code: u32,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub errors: Option<serde_json::Value>,
    #[serde(default)]
    pub captcha_key: Option<Vec<String>>,
    #[serde(default)]
    pub captcha_sitekey: Option<String>,
    #[serde(default)]
    pub captcha_service: Option<String>,
    #[serde(default)]
    pub captcha_rqdata: Option<String>,
    #[serde(default)]
    pub captcha_rqtoken: Option<String>,
    #[serde(default)]
    pub captcha_session_id: Option<String>,
    #[serde(default)]
    pub should_serve_invisible: bool,
}

impl ApiErrorBody {
    pub fn field_error(&self, field: &str) -> Option<String> {
        let errs = self.errors.as_ref()?.get(field)?.get("_errors")?.as_array()?;
        errs.iter().find_map(|e| e.get("message")?.as_str()).filter(|m| !m.is_empty()).map(str::to_owned)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("HTTP {status}: {} (code {})", body.message, body.code)]
    Api { status: u16, body: Box<ApiErrorBody>, raw: String },
    #[error("rate limited for {retry_after:?} (global: {global})")]
    RateLimited { retry_after: Duration, global: bool },
    #[error("captcha required")]
    CaptchaRequired { sitekey: Option<String>, body: Box<ApiErrorBody> },
    #[error("transport: {0}")]
    Http(#[from] reqwest::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("websocket: {0}")]
    WebSocket(Box<tokio_tungstenite::tungstenite::Error>),
    #[error("zlib: {0}")]
    Zlib(String),
    #[error("gateway closed with fatal code {code}: {reason}")]
    FatalClose { code: u16, reason: String },
    #[error("gateway protocol: {0}")]
    Protocol(String),
    #[error("gateway task stopped")]
    Closed,
    #[error("write blocked: {method} {path}")]
    WriteBlocked { method: String, path: String },
}

impl Error {
    pub fn is_write_blocked(&self) -> bool {
        matches!(self, Error::WriteBlocked { .. })
    }

    pub fn captcha(&self) -> Option<&ApiErrorBody> {
        match self {
            Error::CaptchaRequired { body, .. } => Some(body),
            _ => None,
        }
    }

    pub fn is_unauthorized(&self) -> bool {
        matches!(self, Error::Api { status: 401, .. })
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
