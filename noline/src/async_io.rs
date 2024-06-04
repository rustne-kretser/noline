use crate::error::NolineError;
/// IO wrapper for stdin and stdout
use embedded_io_async::Error;

pub trait ASyncIO {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, NolineError>;
    async fn write(&mut self, buf: &[u8]) -> Result<(), NolineError>;
    async fn flush(&mut self) -> Result<(), NolineError>;
}

pub struct IO<R, W>
where
    R: embedded_io_async::Read,
    W: embedded_io_async::Write,
{
    input: R,
    output: W,
}

impl<R, W> IO<R, W>
where
    R: embedded_io_async::Read,
    W: embedded_io_async::Write,
{
    /// Create IO wrapper from input and output
    pub fn new(input: R, output: W) -> Self {
        Self { input, output }
    }

    /// Consume wrapper and return input and output as tuple
    pub fn take(self) -> (R, W) {
        (self.input, self.output)
    }
}

impl<R, W> ASyncIO for IO<R, W>
where
    R: embedded_io_async::Read,
    W: embedded_io_async::Write,
{
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, NolineError> {
        self.input
            .read(buf).await
            .map_err(|e| NolineError::ReadError(e.kind().into()))
    }

    async fn write(&mut self, buf: &[u8]) -> Result<(), NolineError> {
        self.output
            .write_all(buf).await
            .map_err(|e| NolineError::WriteError(e.kind().into()))
    }

    async fn flush(&mut self) -> Result<(), NolineError> {
        self.output
            .flush().await
            .map_err(|e| NolineError::WriteError(e.kind().into()))
    }
}

#[cfg(any(test, feature = "std"))]
pub mod std_sync {
//    use super::*;
//    use core::fmt;
    use std::io::Read;
    use std::io::Write;

    // Wrapper for std::io::stdin
    pub struct StdinWrapper(std::io::Stdin);
    impl StdinWrapper {
        pub fn new() -> Self {
            Self(std::io::stdin())
        }
    }
    impl Default for StdinWrapper {
        fn default() -> Self {
            Self::new()
        }
    }
    impl embedded_io::ErrorType for StdinWrapper {
        type Error = embedded_io::ErrorKind;
    }
    impl embedded_io::Read for StdinWrapper {
        fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
            let mut b = [0];
            let _ = self
                .0
                .read_exact(&mut b)
                .map_err(|e| Self::Error::from(e.kind()))?;
            buf[0] = b[0];
            Ok(1)
        }
    }
    // Wrapper for std::io::stdout
    pub struct StdoutWrapper(std::io::Stdout);
    impl StdoutWrapper {
        pub fn new() -> Self {
            Self(std::io::stdout())
        }
    }
    impl Default for StdoutWrapper {
        fn default() -> Self {
            Self::new()
        }
    }
    impl embedded_io::ErrorType for StdoutWrapper {
        type Error = embedded_io::ErrorKind;
    }
    impl embedded_io::Write for StdoutWrapper {
        fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
            self.0.write(buf).map_err(|e| e.kind().into())
        }
        fn flush(&mut self) -> Result<(), Self::Error> {
            self.0.flush().map_err(|e| e.kind().into())
        }
    }

    // impl<R, W> fmt::Write for IO<R, W>
    // where
    //     R: embedded_io_async::Read,
    //     W: embedded_io_async::Write,
    // {
    //     fn write_str(&mut self, s: &str) -> fmt::Result {
    //         self.write(s.as_bytes()).await.or(Err(fmt::Error))
    //     }
    // }
}
