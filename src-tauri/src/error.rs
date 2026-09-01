use crate::redact::redact_text;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Message(String),
    UserCancel(String),
    CliMissing(String),
    VersionMismatch { expected: String, actual: String },
    ToolNotInstalled(String),
    CommandFailed(String),
    RecoveryRequired(String),
    Drift(String),
    Unavailable(String),
    Lock(String),
    Io(String),
    Db(String),
    Json(String),
    Invalid(String),
}

impl Error {
    pub fn message(msg: impl Into<String>) -> Self {
        Self::Message(sanitize(&msg.into()))
    }

    pub fn user_cancel(msg: impl Into<String>) -> Self {
        Self::UserCancel(sanitize(&msg.into()))
    }

    pub fn cli_missing(msg: impl Into<String>) -> Self {
        Self::CliMissing(sanitize(&msg.into()))
    }

    pub fn version_mismatch(expected: impl Into<String>, actual: impl Into<String>) -> Self {
        Self::VersionMismatch {
            expected: sanitize(&expected.into()),
            actual: sanitize(&actual.into()),
        }
    }

    pub fn tool_not_installed(msg: impl Into<String>) -> Self {
        Self::ToolNotInstalled(sanitize(&msg.into()))
    }

    pub fn command_failed(msg: impl Into<String>) -> Self {
        Self::CommandFailed(sanitize(&msg.into()))
    }

    pub fn recovery_required(msg: impl Into<String>) -> Self {
        Self::RecoveryRequired(sanitize(&msg.into()))
    }

    pub fn drift(msg: impl Into<String>) -> Self {
        Self::Drift(sanitize(&msg.into()))
    }

    pub fn unavailable(msg: impl Into<String>) -> Self {
        Self::Unavailable(sanitize(&msg.into()))
    }

    pub fn lock(msg: impl Into<String>) -> Self {
        Self::Lock(sanitize(&msg.into()))
    }

    pub fn invalid(msg: impl Into<String>) -> Self {
        Self::Invalid(sanitize(&msg.into()))
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::Message(_) => "message",
            Self::UserCancel(_) => "user-cancel",
            Self::CliMissing(_) => "cli-missing",
            Self::VersionMismatch { .. } => "version-mismatch",
            Self::ToolNotInstalled(_) => "tool-not-installed",
            Self::CommandFailed(_) => "command-failed",
            Self::RecoveryRequired(_) => "recovery-required",
            Self::Drift(_) => "drift",
            Self::Unavailable(_) => "unavailable",
            Self::Lock(_) => "lock",
            Self::Io(_) => "io",
            Self::Db(_) => "db",
            Self::Json(_) => "json",
            Self::Invalid(_) => "invalid",
        }
    }
}

fn sanitize(raw: &str) -> String {
    redact_text(raw)
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let raw = match self {
            Self::Message(msg)
            | Self::UserCancel(msg)
            | Self::CliMissing(msg)
            | Self::ToolNotInstalled(msg)
            | Self::CommandFailed(msg)
            | Self::RecoveryRequired(msg)
            | Self::Drift(msg)
            | Self::Unavailable(msg)
            | Self::Lock(msg)
            | Self::Io(msg)
            | Self::Db(msg)
            | Self::Json(msg)
            | Self::Invalid(msg) => msg.clone(),
            Self::VersionMismatch { expected, actual } => {
                format!("version mismatch: expected {expected}, got {actual}")
            }
        };
        f.write_str(&redact_text(&raw))
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(sanitize(&error.to_string()))
    }
}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        Self::Db(sanitize(&error.to_string()))
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(sanitize(&error.to_string()))
    }
}

impl From<zip::result::ZipError> for Error {
    fn from(error: zip::result::ZipError) -> Self {
        Self::Invalid(sanitize(&error.to_string()))
    }
}

impl serde::Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("Error", 3)?;
        state.serialize_field("kind", self.kind())?;
        state.serialize_field("message", &self.to_string())?;
        state.serialize_field("ok", &false)?;
        state.end()
    }
}
