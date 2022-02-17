use crate::common;
use crate::error::Error;
use crate::line_buffer::Buffer;
use crate::marker::Async;
use crate::output::OutputItem;

pub type NolineInitializer<'a, B> = common::NolineInitializer<'a, B, Async>;
type Noline<'a, B> = common::Noline<'a, B, Async>;

#[cfg(feature = "tokio")]
pub mod with_tokio {
    use super::*;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn write<W: AsyncWriteExt + Unpin>(
        stdout: &mut W,
        buf: &[u8],
    ) -> Result<(), Error<std::io::Error, std::io::Error>> {
        stdout
            .write_all(buf)
            .await
            .or_else(|err| Error::write_error(err))?;
        Ok(())
    }

    async fn flush<W: AsyncWriteExt + Unpin>(
        stdout: &mut W,
    ) -> Result<(), Error<std::io::Error, std::io::Error>> {
        stdout
            .flush()
            .await
            .or_else(|err| Error::write_error(err))?;
        Ok(())
    }

    async fn read<R: AsyncReadExt + Unpin>(
        stdin: &mut R,
    ) -> Result<u8, Error<std::io::Error, std::io::Error>> {
        Ok(stdin
            .read_u8()
            .await
            .or_else(|err| Error::read_error(err))?)
    }

    pub struct Editor<'a, B>
    where
        B: Buffer,
    {
        noline: Noline<'a, B>,
    }

    impl<'a, B> Editor<'a, B>
    where
        B: Buffer,
    {
        pub async fn new<W: AsyncWriteExt + Unpin, R: AsyncReadExt + Unpin>(
            prompt: &'a str,
            stdin: &mut R,
            stdout: &mut W,
        ) -> Result<Editor<'a, B>, Error<std::io::Error, std::io::Error>> {
            let mut initializer = NolineInitializer::<B>::new(prompt);

            write(stdout, initializer.init()).await?;
            flush(stdout).await?;

            let terminal = loop {
                let byte = read(stdin).await?;

                match initializer.advance(byte) {
                    common::InitializerResult::Continue => (),
                    common::InitializerResult::Item(terminal) => break terminal,
                    common::InitializerResult::InvalidInput => return Err(Error::ParserError),
                }
            };

            Ok(Editor {
                noline: Noline::new(prompt, terminal),
            })
        }

        pub async fn readline<'b, W: AsyncWriteExt + Unpin, R: AsyncReadExt + Unpin>(
            &'b mut self,
            stdin: &mut R,
            stdout: &mut W,
        ) -> Result<&'b str, Error<std::io::Error, std::io::Error>> {
            for output in self.noline.reset_line() {
                write(stdout, output.get_bytes().unwrap_or_else(|| unreachable!())).await?;
            }

            flush(stdout).await?;

            let end_of_string = 'outer: loop {
                let b = read(stdin).await?;

                for item in self.noline.input_byte(b) {
                    if let Some(bytes) = item.get_bytes() {
                        write(stdout, bytes).await?;
                    }

                    match item {
                        OutputItem::EndOfString => break 'outer true,
                        OutputItem::Abort => break 'outer false,
                        _ => (),
                    }
                }

                flush(stdout).await?;
            };

            flush(stdout).await?;

            if end_of_string {
                Ok(self.noline.buffer.as_str())
            } else {
                Err(Error::Aborted)
            }
        }
    }
}
