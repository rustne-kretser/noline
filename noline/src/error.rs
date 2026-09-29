//! Error types

/// Enum to hold various error types
#[derive(Debug)]
pub enum NolineError {
    ParserError,
    Aborted,
    IoError(embedded_io::ErrorKind),
}

impl<E> From<E> for NolineError
where
    E: embedded_io::Error,
{
    fn from(value: E) -> Self {
        NolineError::IoError(value.kind())
    }
}
