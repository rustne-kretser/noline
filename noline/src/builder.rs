//! Builder for editors

use core::marker::PhantomData;

use crate::{
    async_editor,
    error::NolineError,
    history::{History, NoHistory, SliceHistory},
    line_buffer::{Buffer, LineBuffer, NoBuffer, SliceBuffer},
    sync_editor,
};

#[cfg(any(test, doc, feature = "alloc", feature = "std"))]
use crate::{history::UnboundedHistory, line_buffer::UnboundedBuffer};

/// Builder for [`sync_editor::Editor`] and [`async_editor::Editor`].
///
/// # Example
/// ```no_run
/// # use embedded_io::{Read, Write, ErrorType};
/// # use core::convert::Infallible;
/// # struct MyIO {}
/// # impl ErrorType for MyIO {
/// #     type Error = Infallible;
/// # }
/// # impl embedded_io::Write for MyIO {
/// #     fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> { unimplemented!() }
/// #     fn flush(&mut self) -> Result<(), Self::Error> { unimplemented!() }
/// # }
/// # impl embedded_io::Read for MyIO {
/// #     fn read(&mut self, buf: &mut[u8]) -> Result<usize, Self::Error> { unimplemented!() }
/// # }
/// # let mut io = MyIO {};
/// use noline::builder::EditorBuilder;
///
/// let mut buffer = [0; 100];
/// let mut history = [0; 200];
/// let mut editor = EditorBuilder::from_slice(&mut buffer)
///     .with_slice_history(&mut history)
///     .build_sync(&mut io)
///     .unwrap();
/// ```
pub struct EditorBuilder<B: Buffer, H: History> {
    line_buffer: LineBuffer<B>,
    history: H,
    _marker: PhantomData<(B, H)>,
}

impl EditorBuilder<NoBuffer, NoHistory> {
    /// Create builder for editor with static buffer
    ///
    /// # Example
    /// ```
    /// use noline::builder::EditorBuilder;
    ///
    /// let mut buffer = [0; 100];
    /// let builder = EditorBuilder::from_slice(&mut buffer);
    /// ```
    pub fn from_slice(buffer: &mut [u8]) -> EditorBuilder<SliceBuffer<'_>, NoHistory> {
        EditorBuilder {
            line_buffer: LineBuffer::from_slice(buffer),
            history: NoHistory {},
            _marker: PhantomData,
        }
    }

    #[cfg(any(test, doc, feature = "alloc", feature = "std"))]
    /// Create builder for editor with unbounded buffer
    ///
    /// # Example
    /// ```
    /// use noline::builder::EditorBuilder;
    ///
    /// let builder = EditorBuilder::new_unbounded();
    /// ```
    pub fn new_unbounded() -> EditorBuilder<UnboundedBuffer, NoHistory> {
        EditorBuilder {
            line_buffer: LineBuffer::new_unbounded(),
            history: NoHistory {},
            _marker: PhantomData,
        }
    }
}

impl<B: Buffer, H: History> EditorBuilder<B, H> {
    /// Add static history
    pub fn with_slice_history(self, buffer: &mut [u8]) -> EditorBuilder<B, SliceHistory<'_>> {
        EditorBuilder {
            line_buffer: self.line_buffer,
            history: SliceHistory::new(buffer),
            _marker: PhantomData,
        }
    }

    #[cfg(any(test, feature = "alloc", feature = "std"))]
    /// Add unbounded history
    pub fn with_unbounded_history(self) -> EditorBuilder<B, UnboundedHistory> {
        EditorBuilder {
            line_buffer: self.line_buffer,
            history: UnboundedHistory::new(),
            _marker: PhantomData,
        }
    }

    /// Build [`sync_editor::Editor`]. Is equivalent of calling [`sync_editor::Editor::new()`].
    pub fn build_sync<IO: embedded_io::Read + embedded_io::Write>(
        self,
        io: &mut IO,
    ) -> Result<sync_editor::Editor<B, H>, NolineError> {
        sync_editor::Editor::new(self.line_buffer, self.history, io)
    }

    /// Build [`async_editor::Editor`]. Is equivalent of calling [`async_editor::Editor::new()`].
    pub async fn build_async<IO: embedded_io_async::Read + embedded_io_async::Write>(
        self,
        io: &mut IO,
    ) -> Result<async_editor::Editor<B, H>, NolineError> {
        async_editor::Editor::new(self.line_buffer, self.history, io).await
    }
}
