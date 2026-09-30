//! Asynchronous IO for the editor.

use embedded_io_async::{Read, ReadExactError, Write};

use crate::{
    core::Prompt,
    editor::Editor,
    error::NolineError,
    history::History,
    line_buffer::Buffer,
    output::{Output, OutputItem},
};

impl<B: Buffer, H: History> Editor<B, H> {
    /// Read a line from the supplied I/O
    ///
    /// Dropping this future may interrupt output. Use [`crate::editor`] to retain
    /// a session across separately scheduled input packets.
    pub async fn readline_async<'b, 'item, IO, I>(
        &'b mut self,
        prompt: impl Into<Prompt<I>>,
        io: &mut IO,
    ) -> Result<&'b str, NolineError>
    where
        IO: Read + Write,
        I: Iterator<Item = &'item str> + Clone,
    {
        let mut line = self.session(prompt);
        handle_output(line.start(), io).await?;

        loop {
            let mut byte = [0];
            io.read_exact(&mut byte)
                .await
                .map_err(|error| match error {
                    ReadExactError::UnexpectedEof => NolineError::Aborted,
                    ReadExactError::Other(error) => error.into(),
                })?;

            if handle_output(line.advance(byte[0]), io).await? {
                break;
            }
        }

        Ok(self.buffer.as_str())
    }
}

async fn handle_output<'b, 'item, B, IO, I>(
    output: Output<'b, B, I>,
    io: &mut IO,
) -> Result<bool, NolineError>
where
    B: Buffer,
    IO: Write,
    I: Iterator<Item = &'item str> + Clone,
{
    for item in output {
        if let Some(bytes) = item.get_bytes() {
            io.write_all(bytes).await?;
        }

        io.flush().await?;

        match item {
            OutputItem::EndOfString => return Ok(true),
            OutputItem::Abort => return Err(NolineError::Aborted),
            _ => (),
        }
    }

    Ok(false)
}
