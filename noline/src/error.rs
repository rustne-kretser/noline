//! Error types

/// Enum to hold various error types
#[derive(Debug)]
pub enum NolineError {
    ParserError,
    Aborted,
    ReadError(embedded_io::ErrorKind),
    WriteError(embedded_io::ErrorKind),
}

impl embedded_io::Error for NolineError {
    fn kind(&self) -> embedded_io::ErrorKind {
        match *self {
            NolineError::ParserError => embedded_io::ErrorKind::InvalidData,
            NolineError::Aborted => embedded_io::ErrorKind::Interrupted,
            NolineError::ReadError(e) => e.kind(),
            NolineError::WriteError(e) => e.kind(),
        }
    }
}
