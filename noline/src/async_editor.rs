//! Line editor for async IO

//! Implementation for async Editor

use embedded_io_async::ReadExactError;

use crate::{
    core::{Line, Prompt},
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
        buffer: LineBuffer<B>,
        history: H,
        _io: &mut IO,
    ) -> Result<Self, NolineError> {
        let terminal = Terminal::default();

        Ok(Self {
            buffer,
            terminal,
            history,
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

    async fn read_byte<IO>(io: &mut IO) -> Result<u8, NolineError>
    where
        IO: embedded_io_async::Read + embedded_io_async::Write,
    {
        let mut buf = [0x8; 1];

        match io.read_exact(&mut buf).await {
            Ok(_) => Ok(buf[0]),
            Err(err) => match err {
                ReadExactError::UnexpectedEof => Err(NolineError::Aborted),
                ReadExactError::Other(err) => Err(err)?,
            },
        }
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

        let mut reset = line.reset();

        Self::handle_output(reset.start(), io).await?;

        loop {
            let byte = Self::read_byte(io).await?;

            if let Some(output) = reset.advance(byte) {
                Self::handle_output(output, io).await?;
            } else {
                break;
            }
        }

        loop {
            let byte = Self::read_byte(io).await?;

            if Self::handle_output(line.advance(byte), io).await?.is_some() {
                break;
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
