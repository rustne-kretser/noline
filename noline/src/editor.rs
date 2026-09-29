//! Line editor with synchronous, asynchronous, and incremental IO.
//!
//! Read complete lines with [`Editor::readline`] or [`Editor::readline_async`].
//! Use [`Editor::session`] when the application manages IO.
//!
//! Keep a session alive between input packets. Start it with terminal probing or
//! known geometry, then feed bytes with [`Session::advance`]. Fully consume and
//! write each output before the next editing operation: consuming it updates the
//! editor's cursor state. After interrupted output or a disconnect, restore the
//! terminal to a state that accepts commands, then start a new session.
//!
//! Display positioning assumes one cell per Unicode scalar; wide and combining
//! characters are not supported.
//!
//! ```
//! use noline::{builder::EditorBuilder, editor::OutputItem};
//! let mut buffer = [0; 64];
//! let mut editor = EditorBuilder::from_slice(&mut buffer).build();
//! let mut session = editor.session("> ");
//! // The caller knows the terminal is on row 3 of a 24-by-80 screen.
//! for item in session.start_at(24, 80, 3).unwrap() {
//!     if let Some(bytes) = item.get_bytes() { /* write all bytes */ }
//! }
//! for packet in [b"get /".as_slice(), b"value\r"] {
//!     for &byte in packet {
//!         for item in session.advance(byte) {
//!             if let Some(bytes) = item.get_bytes() { /* write all bytes */ }
//!             if matches!(item, OutputItem::EndOfString) { /* line submitted */ }
//!         }
//!     }
//! }
//! assert_eq!(session.as_str(), "get /value");
//! ```

pub use crate::core::{Line as Session, Prompt, StrIter};
pub use crate::output::{Output, OutputItem};

use crate::{
    history::{get_history_entries, CircularSlice, History},
    line_buffer::{Buffer, LineBuffer},
    terminal::Terminal,
};

/// Owns line storage, history, and terminal state between sessions.
pub struct Editor<B: Buffer, H: History> {
    pub(crate) buffer: LineBuffer<B>,
    history: H,
    terminal: Terminal,
}

impl<B: Buffer, H: History> Editor<B, H> {
    /// Create an editor without performing I/O.
    pub fn new(buffer: LineBuffer<B>, history: H) -> Self {
        Self {
            buffer,
            history,
            terminal: Terminal::default(),
        }
    }

    /// Borrow the editor for one input line. Start the session before feeding input.
    pub fn session<'a, 'item, I>(&'a mut self, prompt: impl Into<Prompt<I>>) -> Session<'a, B, H, I>
    where
        I: Iterator<Item = &'item str> + Clone + 'a,
    {
        Session::new(
            prompt,
            &mut self.buffer,
            &mut self.terminal,
            &mut self.history,
        )
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
