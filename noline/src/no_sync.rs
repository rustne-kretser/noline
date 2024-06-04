//! Line editor for async IO

#[cfg(any(test, doc, feature = "tokio"))]
pub mod tokio {
    //! Implementation for tokio

    use crate::{
        async_io::ASyncIO,
        core::{Initializer, InitializerResult, Line},
        error::NolineError,
        history::{get_history_entries, CircularSlice, History},
        line_buffer::{Buffer, LineBuffer},
        output::OutputItem,
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
        pub async fn new(
            read: &mut dyn embedded_io_async::Read,
            write: &mut dyn embedded_io_async::Write,
        ) -> Result<Self, NolineError> {
            let mut initializer = Initializer::new();

            write.write(Initializer::init()).await?;
            write.flush().await?;

            let terminal = loop {
                let byte = read.read().await?;

                match initializer.advance(byte) {
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

        /// Read line from `stdin`
        pub async fn readline<'b>(
            &'b mut self,
            prompt: &str,
            io: &mut dyn ASyncIO,
        ) -> Result<&'b str, NolineError> {
            let mut line = Line::new(
                prompt,
                &mut self.buffer,
                &mut self.terminal,
                &mut self.history,
            );

            for output in line.reset() {
                io.write(output.get_bytes().unwrap_or_else(|| unreachable!())).await?;
            }

            io.flush().await?;

            let end_of_string = 'outer: loop {
                let b = io.read().await?;

                for item in line.advance(b) {
                    if let Some(bytes) = item.get_bytes() {
                        io.write(bytes).await?;
                    }

                    match item {
                        OutputItem::EndOfString => break 'outer true,
                        OutputItem::Abort => break 'outer false,
                        _ => (),
                    }
                }

                io.flush().await?;
            };

            io.flush().await?;

            if end_of_string {
                Ok(self.buffer.as_str())
            } else {
                Err(NolineError::Aborted)
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
