use std::fmt;

#[derive(Debug)]
pub enum RpaError {
    Io(std::io::Error),
    InvalidHeader(String),
    UnsupportedVersion(String),
    DecompressionFailed(String),
    PickleFailed(String),
    EntryNotFound(String),
    PackError(String),
}

impl fmt::Display for RpaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RpaError::Io(e) => write!(f, "IO error: {}", e),
            RpaError::InvalidHeader(msg) => write!(f, "Invalid header: {}", msg),
            RpaError::UnsupportedVersion(v) => write!(f, "Unsupported version: {}", v),
            RpaError::DecompressionFailed(msg) => write!(f, "Decompression failed: {}", msg),
            RpaError::PickleFailed(msg) => write!(f, "Pickle deserialization failed: {}", msg),
            RpaError::EntryNotFound(name) => write!(f, "Entry not found: {}", name),
            RpaError::PackError(msg) => write!(f, "Pack error: {}", msg),
        }
    }
}

impl std::error::Error for RpaError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RpaError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for RpaError {
    fn from(e: std::io::Error) -> Self {
        RpaError::Io(e)
    }
}
