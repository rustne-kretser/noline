//! Builder for editors

use crate::{
    editor::Editor,
    history::{History, NoHistory, SliceHistory},
    line_buffer::{Buffer, LineBuffer, NoBuffer, SliceBuffer},
};

#[cfg(any(test, doc, feature = "alloc", feature = "std"))]
use crate::{history::UnboundedHistory, line_buffer::UnboundedBuffer};

/// Builder for [`Editor`].
///
/// # Example
/// ```no_run
/// use noline::builder::EditorBuilder;
///
/// let mut buffer = [0; 100];
/// let mut history = [0; 200];
/// let mut editor = EditorBuilder::from_slice(&mut buffer)
///     .with_slice_history(&mut history)
///     .build();
/// ```
pub struct EditorBuilder<B: Buffer, H: History> {
    line_buffer: LineBuffer<B>,
    history: H,
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
        }
    }
}

impl<B: Buffer, H: History> EditorBuilder<B, H> {
    /// Build an editor without performing IO.
    pub fn build(self) -> Editor<B, H> {
        Editor::new(self.line_buffer, self.history)
    }

    /// Add static history
    pub fn with_slice_history(self, buffer: &mut [u8]) -> EditorBuilder<B, SliceHistory<'_>> {
        EditorBuilder {
            line_buffer: self.line_buffer,
            history: SliceHistory::new(buffer),
        }
    }

    #[cfg(any(test, feature = "alloc", feature = "std"))]
    /// Add unbounded history
    pub fn with_unbounded_history(self) -> EditorBuilder<B, UnboundedHistory> {
        EditorBuilder {
            line_buffer: self.line_buffer,
            history: UnboundedHistory::new(),
        }
    }
}
