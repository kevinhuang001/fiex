//! Typed filesystem failures shared by every Core adapter. The string prefix
//! is retained for older wire callers while internal policy checks match on
//! the structured kind instead of parsing messages.
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FsErrorKind {
    Unsupported,
    PermissionDenied,
    ScopeDenied,
    NotFound,
    Offline,
    Conflict,
    StaleRef,
    Cancelled,
    MountNotFound,
    InvalidVfsUri,
    Internal,
}

impl FsErrorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unsupported => "Unsupported",
            Self::PermissionDenied => "PermissionDenied",
            Self::ScopeDenied => "ScopeDenied",
            Self::NotFound => "NotFound",
            Self::Offline => "Offline",
            Self::Conflict => "Conflict",
            Self::StaleRef => "StaleRef",
            Self::Cancelled => "Cancelled",
            Self::MountNotFound => "MountNotFound",
            Self::InvalidVfsUri => "InvalidVfsUri",
            Self::Internal => "Internal",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        [
            Self::Unsupported,
            Self::PermissionDenied,
            Self::ScopeDenied,
            Self::NotFound,
            Self::Offline,
            Self::Conflict,
            Self::StaleRef,
            Self::Cancelled,
            Self::MountNotFound,
            Self::InvalidVfsUri,
        ]
        .into_iter()
        .find(|kind| kind.as_str() == value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FsError {
    pub kind: FsErrorKind,
    pub message: String,
}

impl FsError {
    pub fn new(kind: FsErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(FsErrorKind::Internal, message)
    }
}

impl fmt::Display for FsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.kind == FsErrorKind::Internal {
            return formatter.write_str(&self.message);
        }
        if self.message.is_empty() {
            formatter.write_str(self.kind.as_str())
        } else {
            write!(formatter, "{}: {}", self.kind.as_str(), self.message)
        }
    }
}

impl std::error::Error for FsError {}

impl From<String> for FsError {
    fn from(message: String) -> Self {
        if let Some((prefix, rest)) = message.split_once(": ") {
            if let Some(kind) = FsErrorKind::parse(prefix) {
                return Self::new(kind, rest);
            }
        }
        Self::internal(message)
    }
}

impl From<&str> for FsError {
    fn from(message: &str) -> Self {
        Self::from(message.to_string())
    }
}

impl From<FsError> for String {
    fn from(error: FsError) -> Self {
        error.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixed_wire_errors_keep_their_kind_and_text() {
        let error = FsError::from("StaleRef: USB mount changed".to_string());
        assert_eq!(error.kind, FsErrorKind::StaleRef);
        assert_eq!(error.to_string(), "StaleRef: USB mount changed");
        let plain = FsError::from("Not a directory (os error 20)".to_string());
        assert_eq!(plain.kind, FsErrorKind::Internal);
        assert_eq!(plain.to_string(), "Not a directory (os error 20)");
    }
}
