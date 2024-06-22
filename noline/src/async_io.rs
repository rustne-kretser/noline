use crate::error::NolineError;
/// IO wrapper for stdin and stdout
use embedded_io_async::Error;

pub struct IO<'a, R, W>
where
    R: embedded_io_async::Read,
    W: embedded_io_async::Write,
{
    input: &'a mut R,
    output: &'a mut W,
}

impl<'a, R, W> IO<'a, R, W>
where
    R: embedded_io_async::Read,
    W: embedded_io_async::Write,
{
    /// Create IO wrapper from input and output
    pub fn new(input: &'a mut R, output: &'a mut W) -> Self {
        Self { input, output }
    }

    /// Consume wrapper and return input and output as tuple
    pub fn take(self) -> (&'a mut R, &'a mut W) {
        (self.input, self.output)
    }

    pub async fn read(&mut self, buf: &mut [u8]) -> Result<usize, NolineError> {
        self.input
            .read(buf)
            .await
            .map_err(|e| NolineError::ReadError(e.kind().into()))
    }

    pub async fn write(&mut self, buf: &[u8]) -> Result<(), NolineError> {
        self.output
            .write_all(buf)
            .await
            .map_err(|e| NolineError::WriteError(e.kind().into()))
    }

    pub async fn flush(&mut self) -> Result<(), NolineError> {
        self.output
            .flush()
            .await
            .map_err(|e| NolineError::WriteError(e.kind().into()))
    }
}

#[cfg(feature = "std")]
pub mod async_std {
    //    use super::*;
    //    use core::fmt;
    use async_std::io::{stdin, stdout, ReadExt, Stdin, Stdout, WriteExt};

    // Wrapper for std::io::stdin
    pub struct StdinWrapper(Stdin);
    impl StdinWrapper {
        pub fn new() -> Self {
            Self(stdin())
        }
    }
    impl Default for StdinWrapper {
        fn default() -> Self {
            Self::new()
        }
    }
    impl embedded_io_async::ErrorType for StdinWrapper {
        type Error = embedded_io_async::ErrorKind;
    }
    impl embedded_io_async::Read for StdinWrapper {
        async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
            let mut b = [0];
            let _ = self
                .0
                .read_exact(&mut b)
                .await
                .map_err(|e| Self::Error::from(e.kind()))?;
            buf[0] = b[0];
            Ok(1)
        }
    }
    // Wrapper for std::io::stdout
    pub struct StdoutWrapper(Stdout);
    impl StdoutWrapper {
        pub fn new() -> Self {
            Self(stdout())
        }
    }
    impl Default for StdoutWrapper {
        fn default() -> Self {
            Self::new()
        }
    }
    impl embedded_io_async::ErrorType for StdoutWrapper {
        type Error = embedded_io_async::ErrorKind;
    }
    impl embedded_io_async::Write for StdoutWrapper {
        async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
            self.0.write(buf).await.map_err(|e| e.kind().into())
        }
        async fn flush(&mut self) -> Result<(), Self::Error> {
            self.0.flush().await.map_err(|e| e.kind().into())
        }
    }
}
