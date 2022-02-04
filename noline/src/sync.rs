use crate::line_buffer::Buffer;
use crate::marker::Sync;

use crate::common;
use crate::common::NolineInitializerState;
use crate::output::OutputItem;

impl<'a, B: Buffer> common::NolineInitializer<'a, B, Sync> {
    pub fn initialize(
        mut self,
        mut input: impl FnMut() -> Result<u8, ()>,
        mut output: impl FnMut(&'a [u8]) -> Result<(), ()>,
    ) -> Result<Noline<'a, B>, ()> {
        output(self.prompt.as_bytes())?;
        output(self.init_bytes())?;

        let terminal = loop {
            if let NolineInitializerState::Done(terminal) = self.state {
                break terminal;
            }

            let byte = input()?;
            self.advance(byte)?;
        };

        Ok(Noline::new(self.buffer, self.prompt, terminal))
    }
}

pub type NolineInitializer<'a, B> = common::NolineInitializer<'a, B, Sync>;

impl<'a, B: Buffer> common::Noline<'a, B, Sync> {
    pub fn advance<'b>(
        &'b mut self,
        input: u8,
        mut f: impl FnMut(&[u8]) -> Result<(), ()>,
    ) -> Option<Result<(), ()>> {
        for item in self.input_byte(input) {
            if let Some(bytes) = item.get_bytes() {
                if f(bytes).is_err() {
                    return Some(Err(()));
                }
            }

            match item {
                OutputItem::EndOfString => return Some(Ok(())),
                OutputItem::Abort => return Some(Err(())),
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

    pub fn readline<'a, B: Buffer, W: Write, R: Read>(
        buffer: &'a mut LineBuffer<B>,
        prompt: &'a str,
        stdin: &mut R,
        stdout: &mut W,
    ) -> Result<&'a str, ()> {
        let mut noline = NolineInitializer::new(buffer, prompt).initialize(
            || {
                if let Some(Ok(b)) = stdin.bytes().next() {
                    Ok(b)
                } else {
                    Err(())
                }
            },
            |bytes| {
                stdout.write_all(bytes).or(Err(()))?;
                stdout.flush().or(Err(()))?;
                Ok(())
            },
        )?;

        for i in stdin.bytes() {
            if let Ok(byte) = i {
                match noline.advance(byte, |output| {
                    stdout.write(output).or(Err(()))?;
                    Ok(())
                }) {
                    Some(rc) => {
                        if rc.is_ok() {
                            return Ok(noline.buffer.as_str());
                        } else {
                            return Err(());
                        }
                    }
                    None => stdout.flush().or(Err(()))?,
                }
            }
        }
        unreachable!();
    }
}

#[cfg(any(test, feature = "embedded"))]
pub mod embedded {
    use super::*;
    use crate::line_buffer::LineBuffer;
    use embedded_hal::serial::{Read, Write};
    use nb::block;

    fn write<W: Write<u8>>(tx: &mut W, buf: &[u8]) -> Result<(), ()> {
        for b in buf {
            block!(tx.write(*b)).or(Err(()))?;
        }

        block!(tx.flush()).or(Err(()))?;

        Ok(())
    }

    pub fn readline<'a, B: Buffer, W: Write<u8>, R: Read<u8>>(
        buffer: &'a mut LineBuffer<B>,
        prompt: &'a str,
        rx: &mut R,
        tx: &mut W,
    ) -> Result<&'a str, ()> {
        let mut noline = NolineInitializer::new(buffer, prompt).initialize(
            || {
                if let Ok(b) = block!(rx.read()) {
                    Ok(b)
                } else {
                    Err(())
                }
            },
            |bytes| {
                write(tx, bytes)?;
                Ok(())
            },
        )?;

        while let Ok(b) = block!(rx.read()) {
            match noline.advance(b, |output| {
                write(tx, output)?;
                Ok(())
            }) {
                Some(rc) => {
                    if rc.is_ok() {
                        return Ok(noline.buffer.as_str());
                    } else {
                        return Err(());
                    }
                }
                None => (),
            }
        }

        return Err(());
    }
}
