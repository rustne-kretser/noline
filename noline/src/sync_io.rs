use crate::error::NolineError;
/// IO wrapper for stdin and stdout
use embedded_io::Error;

pub struct IO<RW>
where
    RW: embedded_io::Read + embedded_io::Write,
{
    rw: RW,
}

impl<RW> IO<RW>
where
    RW: embedded_io::Read + embedded_io::Write,
{
    pub fn new(rw: RW) -> Self {
        Self { rw }
    }

    pub fn inner(&mut self) -> &mut RW {
        &mut self.rw
    }
}

impl<RW> embedded_io::ErrorType for IO<RW>
where
    RW: embedded_io::Read + embedded_io::Write,
{
    type Error = NolineError;
}

impl<RW> embedded_io::Read for IO<RW>
where
    RW: embedded_io::Read + embedded_io::Write,
{
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, NolineError> {
        self.rw
            .read(buf)
            .map_err(|e| NolineError::ReadError(e.kind().into()))
    }
}

impl<RW> embedded_io::Write for IO<RW>
where
    RW: embedded_io::Read + embedded_io::Write,
{
    fn write(&mut self, buf: &[u8]) -> Result<usize, NolineError> {
        self.rw
            .write_all(buf)
            .map_err(|e| NolineError::WriteError(e.kind().into()))?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<(), NolineError> {
        self.rw
            .flush()
            .map_err(|e| NolineError::WriteError(e.kind().into()))
    }
}

impl<RW> core::fmt::Write for IO<RW>
where
    RW: embedded_io::Read + embedded_io::Write,
{
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.rw.write(s.as_bytes()).or(Err(core::fmt::Error))?;
        Ok(())
    }
}

#[cfg(any(test, feature = "std"))]
pub mod std_sync {
    use super::*;
    use core::fmt;
    use std::io::{Read, Stdin, Stdout, Write};

    // Wrapper for std::io::stdin
    pub struct StdinWrapper(std::io::Stdin);
    impl StdinWrapper {
        pub fn new() -> Self {
            Self(std::io::stdin())
        }
        pub fn new_with(val: Stdin) -> Self {
            Self(val)
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
        pub fn new_with(val: Stdout) -> Self {
            Self(val)
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
}
