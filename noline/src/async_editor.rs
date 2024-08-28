//! Line editor for async IO

//! Implementation for async Editor

use embedded_io_async::ReadExactError;

use crate::{
    core::{Initializer, InitializerResult, Line, Prompt},
    error::NolineError,
    history::{get_history_entries, CircularSlice, History},
    line_buffer::{Buffer, LineBuffer},
    output::{Output, OutputItem},
    terminal::Terminal,
};

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
    pub async fn new<IO: embedded_io_async::Read + embedded_io_async::Write>(
        io: &mut IO,
    ) -> Result<Self, NolineError> {
        let mut initializer = Initializer::new();

        io.write(Initializer::init()).await?;

        io.flush().await?;

        let terminal = loop {
            let mut buf = [0u8; 1];
            // let len = io.read_exact(&mut buf).await?;

            match io.read_exact(&mut buf).await {
                Ok(_) => (),
                Err(err) => match err {
                    ReadExactError::UnexpectedEof => return Err(NolineError::Aborted),
                    ReadExactError::Other(err) => Err(err)?,
                },
            }

            match initializer.advance(buf[0]) {
                InitializerResult::Continue => (),
                InitializerResult::Item(terminal) => break terminal,
                InitializerResult::InvalidInput => return Err(NolineError::ParserError),
            }
        };

        Ok(Self {
            buffer: LineBuffer::new(),
            terminal,
            history: H::default(),
        })
    }

    async fn handle_output<'b, 'item, IO, I>(
        output: Output<'b, B, I>,
        io: &mut IO,
    ) -> Result<Option<()>, NolineError>
    where
        IO: embedded_io_async::Read + embedded_io_async::Write,
        I: Iterator<Item = &'item str> + Clone,
    {
        for item in output {
            if let Some(bytes) = item.get_bytes() {
                io.write(bytes).await?;
            }

            io.flush().await?;

            match item {
                OutputItem::EndOfString => return Ok(Some(())),
                OutputItem::Abort => return Err(NolineError::Aborted),
                _ => (),
            }
        }

        Ok(None)
    }

    /// Read line from `stdin`
    pub async fn readline<'b, 'item, IO, I>(
        &'b mut self,
        prompt: impl Into<Prompt<I>>,
        io: &mut IO,
    ) -> Result<&'b str, NolineError>
    where
        IO: embedded_io_async::Read + embedded_io_async::Write,
        I: Iterator<Item = &'item str> + Clone,
    {
        let mut line = Line::new(
            prompt,
            &mut self.buffer,
            &mut self.terminal,
            &mut self.history,
        );
        Self::handle_output(line.reset(), io).await?;

        loop {
            let mut buf = [0x8; 1];

            match io.read_exact(&mut buf).await {
                Ok(_) => {
                    if Self::handle_output(line.advance(buf[0]), io)
                        .await?
                        .is_some()
                    {
                        break;
                    }
                }
                Err(err) => match err {
                    ReadExactError::UnexpectedEof => return Err(NolineError::Aborted),
                    ReadExactError::Other(err) => Err(err)?,
                },
            }
        }

        Ok(self.buffer.as_str())
    }

    /// Load history from iterator
    pub fn load_history<'a>(&mut self, entries: impl Iterator<Item = &'a str>) -> usize {
        self.history.load_entries(entries)
    }

    /// Get history as iterator over circular slices
    pub fn get_history(&self) -> impl Iterator<Item = CircularSlice<'_>> {
        get_history_entries(&self.history)
    }
}
