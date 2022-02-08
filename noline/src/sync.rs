use crate::error::Error;
use crate::line_buffer::Buffer;
use crate::marker::Sync;

use crate::common;
use crate::output::OutputItem;

impl<'a, B: Buffer> common::NolineInitializer<'a, B, Sync> {
    pub fn initialize<E>(
        mut self,
        mut input: impl FnMut() -> Result<u8, Error<E>>,
        mut output: impl FnMut(&'a [u8]) -> Result<(), Error<E>>,
    ) -> Result<Noline<'a, B>, Error<E>> {
        output(self.clear_line())?;
        output(self.prompt.as_bytes())?;
        output(self.probe_size())?;

        let terminal = loop {
            let byte = input()?;

            match self.advance(byte) {
                common::InitializerResult::Continue => (),
                common::InitializerResult::Item(terminal) => break terminal,
                common::InitializerResult::InvalidInput => return Err(Error::ParserError),
            }
        };

        Ok(Noline::new(self.buffer, self.prompt, terminal))
    }
}

pub type NolineInitializer<'a, B> = common::NolineInitializer<'a, B, Sync>;

impl<'a, B: Buffer> common::Noline<'a, B, Sync> {
    pub fn advance<'b, F, E>(&'b mut self, input: u8, mut f: F) -> Option<Result<(), Error<E>>>
    where
        F: FnMut(&[u8]) -> Result<(), Error<E>>,
    {
        for item in self.input_byte(input) {
            if let Some(bytes) = item.get_bytes() {
                if let Err(err) = f(bytes) {
                    return Some(Err(err));
                }
            }

            match item {
                OutputItem::EndOfString => return Some(Ok(())),
                OutputItem::Abort => return Some(Err(Error::Aborted)),
                _ => (),
            }
        }

        None
    }
}

pub type Noline<'a, B> = common::Noline<'a, B, Sync>;

#[cfg(any(test, feature = "std"))]
pub mod with_std {
    use super::*;
    use crate::line_buffer::LineBuffer;
    use std::io::Read;
    use std::io::Write;

    pub fn readline<'a, B, W, R>(
        buffer: &'a mut LineBuffer<B>,
        prompt: &'a str,
        stdin: &mut R,
        stdout: &mut W,
    ) -> Result<&'a str, Error<std::io::Error>>
    where
        B: Buffer,
        W: Write,
        R: Read,
    {
        let mut noline = NolineInitializer::new(buffer, prompt).initialize(
            || {
                // let b = stdin.bytes().next()?;

                if let Some(b) = stdin.bytes().next() {
                    let b = b?;
                    Ok(b)
                } else {
                    Err(Error::EOF)
                }
            },
            |bytes| {
                stdout.write_all(bytes)?;
                stdout.flush()?;
                Ok(())
            },
        )?;

        for i in stdin.bytes() {
            if let Ok(byte) = i {
                match noline.advance(byte, |output| {
                    stdout.write(output)?;
                    Ok(())
                }) {
                    Some(rc) => {
                        rc?;

                        return Ok(noline.buffer.as_str());
                    }
                    None => (),
                }
            }
        }

        unreachable!();
    }
}

#[cfg(any(test, feature = "embedded"))]
pub mod embedded {
    use core::cell::RefCell;

    use super::*;
    use crate::line_buffer::LineBuffer;
    use embedded_hal::serial::{Read, Write};
    use nb::block;

    pub fn write<W, E>(tx: &mut W, buf: &[u8]) -> Result<(), Error<W::Error>>
    where
        W: Write<u8, Error = E>,
        // E: core::convert::From<<W as embedded_hal::prelude::_embedded_hal_serial_Write<u8>>::Error>,
    {
        for b in buf {
            block!(tx.write(*b))?;
        }

        block!(tx.flush())?;

        Ok(())
    }

    pub fn readline<'a, B, S, E>(
        buffer: &'a mut LineBuffer<B>,
        prompt: &'a str,
        serial: &mut S,
    ) -> Result<&'a str, Error<E>>
    where
        B: Buffer,
        S: Write<u8, Error = E> + Read<u8, Error = E>,
    {
        let serial = RefCell::new(serial);

        let mut noline = NolineInitializer::new(buffer, prompt).initialize(
            || Ok(block!(serial.borrow_mut().read())?),
            |bytes| {
                write(*serial.borrow_mut(), bytes)?;
                Ok(())
            },
        )?;

        let serial = serial.into_inner();

        loop {
            let b = block!(serial.read())?;

            match noline.advance::<_, E>(b, |output| {
                write(serial, output)?;
                Ok(())
            }) {
                Some(rc) => {
                    if rc.is_ok() {
                        break Ok(noline.buffer.as_str());
                    } else {
                        break Err(Error::ParserError);
                    }
                }
                None => (),
            }
        }

    }
}
