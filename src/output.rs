/// Unified JSON output for all community-agent commands.
///
/// Every command prints exactly one JSON object to stdout:
///   { ok, user_message, data, error, _internal }
///
/// Diagnostic logs go to stderr via log_* macros. Set
/// COMMUNITY_DEBUG=1 for verbose output.

use serde::Serialize;
use serde_json::Value;

pub fn is_debug() -> bool {
    matches!(
        std::env::var("COMMUNITY_DEBUG").as_deref(),
        Ok("1") | Ok("true")
    )
}

#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        if $crate::output::is_debug() {
            eprintln!("[community-agent DEBUG] {}", format!($($arg)*));
        }
    };
}
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        eprintln!("[community-agent] {}", format!($($arg)*));
    };
}
#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        eprintln!("[community-agent WARN] {}", format!($($arg)*));
    };
}
#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        eprintln!("[community-agent ERROR] {}", format!($($arg)*));
    };
}

#[derive(Serialize, Default)]
pub struct Internal {
    /// What the LLM should do next. Examples:
    ///   "done", "fix_command", "wait_for_claim", "retry", "post_again"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_action: Option<String>,

    /// Concrete next-step command template for the LLM to emit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_command: Option<String>,

    /// Extra hints (free-form).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

#[derive(Serialize)]
pub struct ErrorBody {
    pub code: String,
    pub domain: String,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    #[serde(skip_serializing_if = "Value::is_null")]
    pub debug: Value,
}

#[derive(Serialize)]
pub struct Output {
    pub ok: bool,
    pub user_message: String,
    #[serde(skip_serializing_if = "Value::is_null")]
    pub data: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBody>,
    #[serde(rename = "_internal")]
    pub internal: Internal,
}

impl Output {
    pub fn ok(user_message: impl Into<String>, data: Value, internal: Internal) -> Self {
        Self {
            ok: true,
            user_message: user_message.into(),
            data,
            error: None,
            internal,
        }
    }

    pub fn error(
        user_message: impl Into<String>,
        code: impl Into<String>,
        domain: impl Into<String>,
        retryable: bool,
        hint: impl Into<String>,
        internal: Internal,
    ) -> Self {
        Self {
            ok: false,
            user_message: user_message.into(),
            data: Value::Null,
            error: Some(ErrorBody {
                code: code.into(),
                domain: domain.into(),
                retryable,
                hint: Some(hint.into()),
                debug: Value::Null,
            }),
            internal,
        }
    }

    pub fn error_with_debug(
        user_message: impl Into<String>,
        code: impl Into<String>,
        domain: impl Into<String>,
        retryable: bool,
        hint: impl Into<String>,
        debug: Value,
        internal: Internal,
    ) -> Self {
        let mut out = Self::error(user_message, code, domain, retryable, hint, internal);
        if let Some(e) = out.error.as_mut() {
            e.debug = debug;
        }
        out
    }

    pub fn print(&self) {
        match serde_json::to_string(self) {
            Ok(s) => println!("{s}"),
            Err(e) => println!("{{\"ok\":false,\"user_message\":\"output serialization failed: {e}\"}}"),
        }
    }
}
