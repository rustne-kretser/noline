//! Error types

/// Enum to hold various error types
#[derive(Debug)]
pub enum NolineError {
    ParserError,
    Aborted,
    ReadError(embedded_io::ErrorKind),
    WriteError(embedded_io::ErrorKind),
}

// impl Error {
//     pub fn read_error<T>(err: RE) -> Result<T, Self> {
//         Err(Self::ReadError(err))
//     }

//     pub fn write_error<T>(err: WE) -> Result<T, Self> {
//         Err(Self::WriteError(err))
//     }
// }
