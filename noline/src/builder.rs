//! Builder for editors

use core::marker::PhantomData;

use crate::{
    async_editor, error::NolineError, history::{History, NoHistory, StaticHistory}, line_buffer::{Buffer, NoBuffer, StaticBuffer}, sync_editor
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
/// let mut editor = EditorBuilder::new_static::<100>()
///     .with_static_history::<200>()
///     .build_sync(&mut io)
///     .unwrap();
/// ```
pub struct EditorBuilder<B: Buffer, H: History> {
    _marker: PhantomData<(B, H)>,
}

impl EditorBuilder<NoBuffer, NoHistory> {
    /// Create builder for editor with static buffer
    ///
    /// # Example
    /// ```
    /// use noline::builder::EditorBuilder;
    ///
    /// let builder = EditorBuilder::new_static::<100>();
    /// ```
    pub fn new_static<const N: usize>() -> EditorBuilder<StaticBuffer<N>, NoHistory> {
        EditorBuilder {
            _marker: PhantomData,
        }
    }

    #[cfg(any(test, feature = "alloc", feature = "std"))]
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
            _marker: PhantomData,
        }
    }
}

impl<B: Buffer, H: History> EditorBuilder<B, H> {
    /// Add static history
    pub fn with_static_history<const N: usize>(self) -> EditorBuilder<B, StaticHistory<N>> {
        EditorBuilder {
            _marker: PhantomData,
        }
    }

    #[cfg(any(test, feature = "alloc", feature = "std"))]
    /// Add unbounded history
    pub fn with_unbounded_history(self) -> EditorBuilder<B, UnboundedHistory> {
        EditorBuilder {
            _marker: PhantomData,
        }
    }

    /// Build [`sync_editor::Editor`]. Is equivalent of calling [`sync_editor::Editor::new()`].
    pub fn build_sync<IO: embedded_io::Read + embedded_io::Write>(
        self,
        io: &mut IO,
    ) -> Result<sync_editor::Editor<B, H>, NolineError> {
        sync_editor::Editor::new(io)
    }

    /// Build [`async_editor::Editor`]. Is equivalent of calling [`async_editor::Editor::new()`].
    pub async fn build_async<IO: embedded_io_async::Read + embedded_io_async::Write>(
        self,
        io: &mut IO,
    ) -> Result<async_editor::Editor<B, H>, NolineError> {
        async_editor::Editor::new(io).await
    }
}
