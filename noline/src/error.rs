//! Error types

/// Enum to hold various error types
#[derive(Debug)]
pub enum NolineError {
    ParserError,
    Aborted,
    IoError(embedded_io::ErrorKind),
}

impl From<crate::editor::Error> for NolineError {
    fn from(_: crate::editor::Error) -> Self {
        Self::ParserError
    }
}
