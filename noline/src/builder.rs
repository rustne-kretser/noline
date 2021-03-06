//! Builder for editors

use core::marker::PhantomData;

use crate::{
    error::Error,
    history::{History, NoHistory, StaticHistory},
    line_buffer::{Buffer, NoBuffer, StaticBuffer},
    sync::{self, Read, Write},
};

#[cfg(any(test, feature = "alloc", feature = "std"))]
use crate::line_buffer::UnboundedBuffer;

#[cfg(any(test, feature = "alloc", feature = "std"))]
use crate::history::UnboundedHistory;

#[cfg(any(test, doc, feature = "tokio"))]
use ::tokio::io::{AsyncReadExt, AsyncWriteExt};

#[cfg(any(test, doc, feature = "tokio"))]
use crate::no_sync;

/// Builder for [`sync::Editor`] and [`no_sync::tokio::Editor`].
///
/// # Example
/// ```no_run
/// # use noline::sync::{Read, Write};
/// # struct IO {}
/// # impl Write for IO {
/// #     type Error = ();
/// #     fn write(&mut self, buf: &[u8]) -> Result<(), Self::Error> { unimplemented!() }
/// #     fn flush(&mut self) -> Result<(), Self::Error> { unimplemented!() }
/// # }
/// # impl Read for IO {
/// #     type Error = ();
/// #     fn read(&mut self) -> Result<u8, Self::Error> { unimplemented!() }
/// # }
/// # let mut io = IO {};
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

    /// Build [`sync::Editor`]. Is equivalent of calling [`sync::Editor::new()`].
    pub fn build_sync<IO, RE, WE>(
        self,
        io: &mut IO,
    ) -> Result<sync::Editor<B, H, IO>, Error<RE, WE>>
    where
        IO: Read<Error = RE> + Write<Error = WE>,
    {
        sync::Editor::new(io)
    }

    #[cfg(any(test, doc, feature = "tokio"))]
    /// Build [`no_sync::tokio::Editor`]. Is equivalent of calling [`no_sync::tokio::Editor::new()`].
    pub async fn build_async_tokio<W: AsyncWriteExt + Unpin, R: AsyncReadExt + Unpin>(
        self,
        stdin: &mut R,
        stdout: &mut W,
    ) -> Result<no_sync::tokio::Editor<B, H>, Error<std::io::Error, std::io::Error>> {
        no_sync::tokio::Editor::new(stdin, stdout).await
    }
}
