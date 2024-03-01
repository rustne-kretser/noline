//! Line editor for async IO

#[cfg(any(test, doc, feature = "tokio"))]
pub mod tokio {
    //! Implementation for tokio

    use crate::{
        core::{Initializer, InitializerResult, Line},
        error::Error,
        history::{get_history_entries, CircularSlice, History},
        line_buffer::{Buffer, LineBuffer},
        output::OutputItem,
        terminal::Terminal,
    };

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

    /// Line editor for async IO
    ///
    /// It is recommended to use [`crate::builder::EditorBuilder`] to build an editor.
    pub struct Editor<B: Buffer, H: History> {
        buffer: LineBuffer<B>,
        terminal: Terminal,
        history: H,
    }

    impl<B, H> Editor<B, H>
    where
        B: Buffer,
        H: History,
    {
        /// Create and initialize line editor
        pub async fn new<W: AsyncWriteExt + Unpin, R: AsyncReadExt + Unpin>(
            stdin: &mut R,
            stdout: &mut W,
        ) -> Result<Editor<B, H>, Error<std::io::Error, std::io::Error>> {
            let mut initializer = Initializer::new();

            write(stdout, Initializer::init()).await?;
            flush(stdout).await?;

            let terminal = loop {
                let byte = read(stdin).await?;

                match initializer.advance(byte) {
                    InitializerResult::Continue => (),
                    InitializerResult::Item(terminal) => break terminal,
                    InitializerResult::InvalidInput => return Err(Error::ParserError),
                }
            };

            Ok(Self {
                buffer: LineBuffer::new(),
                terminal,
                history: H::default(),
            })
        }

        /// Read line from `stdin`
        pub async fn readline<'b, W: AsyncWriteExt + Unpin, R: AsyncReadExt + Unpin>(
            &'b mut self,
            prompt: &str,
            stdin: &mut R,
            stdout: &mut W,
        ) -> Result<&'b str, Error<std::io::Error, std::io::Error>> {
            let mut line = Line::new(
                prompt,
                &mut self.buffer,
                &mut self.terminal,
                &mut self.history,
            );

            for output in line.reset() {
                write(stdout, output.get_bytes().unwrap_or_else(|| unreachable!())).await?;
            }

            flush(stdout).await?;

            let end_of_string = 'outer: loop {
                let b = read(stdin).await?;

                for item in line.advance(b) {
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
                Ok(self.buffer.as_str())
            } else {
                Err(Error::Aborted)
            }
        }

        /// Load history from iterator
        pub fn load_history<'a>(&mut self, entries: impl Iterator<Item = &'a str>) -> usize {
            self.history.load_entries(entries)
        }

        /// Get history as iterator over circular slices
        pub fn get_history<'a>(&'a self) -> impl Iterator<Item = CircularSlice<'a>> {
            get_history_entries(&self.history)
        }
    }
}

#[cfg(any(test, doc, feature = "embedded"))]
#[allow(async_fn_in_trait)]
pub mod async_trait {
    //! Line editor for async IO

    use core::marker::PhantomData;

    use crate::{
        core::{Initializer, InitializerResult, Line},
        error::Error,
        history::{get_history_entries, CircularSlice, History},
        line_buffer::{Buffer, LineBuffer},
        output::OutputItem,
        terminal::Terminal,
    };

    pub trait NolineAsyncWrite<RE, WE> {
        async fn write(&mut self, buf: &[u8]) -> Result<(), Error<RE, WE>>;
        async fn flush(&mut self) -> Result<(), Error<RE, WE>>;
    }

    pub trait NolineAsyncRead<RE, WE> {
        async fn read(&mut self) -> Result<u8, Error<RE, WE>>;
    }

    /// Line editor for async IO
    ///
    /// It is recommended to use [`crate::builder::EditorBuilder`] to build an editor.
    pub struct AsyncEditor<B: Buffer, H: History, RE, WE> {
        buffer: LineBuffer<B>,
        terminal: Terminal,
        history: H,
        _re: PhantomData<RE>,
        _we: PhantomData<WE>,
    }

    impl<B, H, RE, WE> AsyncEditor<B, H, RE, WE>
    where
        B: Buffer,
        H: History,
    {
        /// Create and initialize line editor
        pub async fn new<R: NolineAsyncRead<RE, WE>, W: NolineAsyncWrite<RE, WE>>(
            stdin: &mut R,
            stdout: &mut W,
        ) -> Result<AsyncEditor<B, H, RE, WE>, Error<RE, WE>> {
            let mut initializer = Initializer::new();

            stdout.write(Initializer::init()).await?;
            stdout.flush().await?;

            let terminal = loop {
                let byte = stdin.read().await?;

                match initializer.advance(byte) {
                    InitializerResult::Continue => (),
                    InitializerResult::Item(terminal) => break terminal,
                    InitializerResult::InvalidInput => return Err(Error::ParserError),
                }
            };

            Ok(Self {
                buffer: LineBuffer::new(),
                terminal,
                history: H::default(),
                _re: PhantomData,
                _we: PhantomData,
            })
        }

        /// Read line from `stdin`
        pub async fn readline<'b, W: NolineAsyncWrite<RE, WE>, R: NolineAsyncRead<RE, WE>>(
            &'b mut self,
            prompt: &str,
            stdin: &mut R,
            stdout: &mut W,
        ) -> Result<&'b str, Error<RE, WE>> {
            let mut line = Line::new(
                prompt,
                &mut self.buffer,
                &mut self.terminal,
                &mut self.history,
            );

            for output in line.reset() {
                stdout
                    .write(output.get_bytes().unwrap_or_else(|| unreachable!()))
                    .await?;
            }

            stdout.flush().await?;

            let end_of_string = 'outer: loop {
                let b = stdin.read().await?;

                for item in line.advance(b) {
                    if let Some(bytes) = item.get_bytes() {
                        stdout.write(bytes).await?;
                    }

                    match item {
                        OutputItem::EndOfString => break 'outer true,
                        OutputItem::Abort => break 'outer false,
                        _ => (),
                    }
                }

                stdout.flush().await?;
            };

            stdout.flush().await?;

            if end_of_string {
                Ok(self.buffer.as_str())
            } else {
                Err(Error::Aborted)
            }
        }

        /// Load history from iterator
        pub fn load_history<'a>(&mut self, entries: impl Iterator<Item = &'a str>) -> usize {
            self.history.load_entries(entries)
        }

        /// Get history as iterator over circular slices
        pub fn get_history<'a>(&'a self) -> impl Iterator<Item = CircularSlice<'a>> {
            get_history_entries(&self.history)
        }
    }
}
