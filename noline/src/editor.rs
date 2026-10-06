//! Application-driven line editing.
//!
//! Use [`crate::sync_editor::Editor::line`] or [`crate::async_editor::Editor::line`]
//! to drive editing yourself, for example to provide completion or print help.
//!
//! The line retains input state between calls. Start each line with terminal
//! probing or known geometry, then feed bytes with [`Line::advance`]. Consume and
//! write all bytes and flush before the next editing operation: consuming output
//! updates the editor's cursor state. After interrupted output or a disconnect,
//! restore the terminal to a state that accepts commands, then start a new line.
//!
//! Use [`Line::suspend`] before application output, then [`Line::resume`] or
//! [`Line::resume_at`] to restore editing. Finish each operation's output before
//! transferring the terminal. Buffer user input during the handoff; terminal
//! queries must be answered before replaying it. The application owns terminal
//! modes and must leave a line available for the prompt.
//! Each line borrows its prompt; [`Line::set_prompt`] changes it while suspended.
//! Cursor and replacement ranges use UTF-8 byte offsets.
//! Display positioning assumes one cell per Unicode scalar; wide and combining
//! characters are not supported.
//! Drafts taller than the terminal are not fully repainted when scrolling back.
//!
//! ```
//! use noline::{builder::EditorBuilder, editor::Event};
//! # fn example(io: &mut (impl embedded_io::Read + embedded_io::Write)) -> Result<(), noline::error::NolineError> {
//! let mut buffer = [0; 64];
//! let mut editor = EditorBuilder::from_slice(&mut buffer).build_sync(io)?;
//! let mut line = editor.line("> ");
//! // The caller knows the terminal is on row 3 of a 24-by-80 screen.
//! for item in line.start_at(24, 80, 3).unwrap() {
//!     if let Some(bytes) = item.get_bytes() { io.write_all(bytes)?; }
//! }
//! io.flush()?;
//! for packet in [b"get /".as_slice(), b"value\r"] {
//!     for &byte in packet {
//!         for item in line.advance(byte).unwrap() {
//!             if let Some(bytes) = item.get_bytes() { io.write_all(bytes)?; }
//!             if item.event() == Some(Event::Submitted) { /* line submitted */ }
//!         }
//!         io.flush()?;
//!     }
//! }
//! assert_eq!(line.as_str(), "get /value");
//! # Ok(()) }
//! ```

pub use crate::core::{Line, Prompt, StrIter};
pub use crate::output::{Event, Output, OutputItem};

/// Invalid editing operation or terminal geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The operation is not available in the current editing phase.
    InvalidState,
    /// Terminal dimensions or cursor row are invalid.
    InvalidGeometry,
}
