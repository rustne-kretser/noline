//! Error types

/// Enum to hold various error types
#[derive(Debug)]
pub enum NolineError {
    ParserError,
    Aborted,
    ReadError(embedded_io::ErrorKind),
    WriteError(embedded_io::ErrorKind),
}
