//! Line editor for async IO

//! Implementation for async Editor

use embedded_io_async::ReadExactError;

use crate::{
    core::{Line, Prompt},
    error::NolineError,
    history::{get_history_entries, CircularSlice, History, HistoryNavigator},
    line_buffer::{Buffer, LineBuffer},
    output::{Event, Output},
    terminal::Terminal,
};

/// Line editor for async IO
///
/// It is recommended to use [`crate::builder::EditorBuilder`] to build an editor.
pub struct Editor<B: Buffer, H: History> {
    buffer: LineBuffer<B>,
    terminal: Terminal,
    nav: HistoryNavigator<H>,
}

impl<B, H> Editor<B, H>
where
    B: Buffer,
    H: History,
{
    /// Create an editor. Terminal initialization occurs when starting a line.
    pub fn new<IO: embedded_io_async::Read + embedded_io_async::Write>(
        buffer: LineBuffer<B>,
        history: H,
        _io: &mut IO,
    ) -> impl core::future::Future<Output = Result<Self, NolineError>> {
        core::future::ready(Ok(Self {
            buffer,
            terminal: Terminal::default(),
            nav: HistoryNavigator::new(history),
        }))
    }

    async fn handle_output<'b, 'item, IO, I>(
        output: Output<'b, B, I>,
        io: &mut IO,
    ) -> Result<Option<()>, NolineError>
    where
        IO: embedded_io_async::Read + embedded_io_async::Write,
        I: Iterator<Item = &'item str> + Clone,
    {
        let mut result = None;
        for item in output {
            if let Some(bytes) = item.get_bytes() {
                io.write_all(bytes).await?;
            }

            result = Some(match item.event() {
                Some(Event::Submitted) => Ok(Some(())),
                Some(Event::Aborted) => Err(NolineError::Aborted),
                _ => Ok(None),
            });
        }
        if result.is_some() {
            io.flush().await?;
        }
        result.unwrap_or(Ok(None))
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

    /// Borrow a line for application-driven editing. Start it before feeding input.
    pub fn line<'item, I>(&mut self, prompt: impl Into<Prompt<I>>) -> Line<'_, B, H, I>
    where
        I: Iterator<Item = &'item str> + Clone,
    {
        Line::new(prompt, &mut self.buffer, &mut self.terminal, &mut self.nav)
    }

    /// Read a line from the supplied I/O.
    ///
    /// Dropping this future may interrupt output. Use [`Self::line`] to drive
    /// individual editing operations instead.
    pub async fn readline<'b, 'item, IO, I>(
        &'b mut self,
        prompt: impl Into<Prompt<I>>,
        io: &mut IO,
    ) -> Result<&'b str, NolineError>
    where
        IO: embedded_io_async::Read + embedded_io_async::Write,
        I: Iterator<Item = &'item str> + Clone,
    {
        let mut line = self.line(prompt);
        Self::handle_output(line.start(), io).await?;

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
        self.nav.history.load_entries(entries)
    }

    /// Get history as iterator over circular slices
    pub fn get_history(&self) -> impl Iterator<Item = CircularSlice<'_>> {
        get_history_entries(&self.nav.history)
    }
}
