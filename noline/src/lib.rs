//! Noline is an IO-agnostic `#[no_std]` line editor providing robust
//! line editing for any system. The core functionality is IO-free, so
//! it can be adapted to any system be it embedded, async, async
//! embedded, WASM or IPoAC (IP over Avian Carriers).
//!
//! Features:
//! - IO-free
//! - Minimal dependencies
//! - No allocation needed - Both heap-based and static buffers are provided
//! - UTF-8 support
//! - Emacs keybindings
//!
//! Possible future features:
//! - Auto-completion and hints
//! - Line history
//!
//! The API should be considered experimental and will change in the
//! future.
//!
//!
//! The core consists of a massive state machine taking bytes as input
//! and returning an iterator over byte slices. There are, however,
//! some convenience wrappers:
//! - [`sync::with_std::Editor`]
//! - [`sync::embedded::Editor`]
//! - [`no_sync::with_tokio::Editor`]
//!
//! # Example
//! ```!
//! use noline::sync::with_std::Editor;
//! use std::io::{self, Write};
//! use termion::raw::IntoRawMode;
//!
//! fn main() {
//!     let mut stdin = io::stdin();
//!     let mut stdout = io::stdout().into_raw_mode().unwrap();
//!     let prompt = "> ";
//!
//!     let mut editor = Editor::<Vec<u8>>::new(prompt, &mut stdin, &mut stdout).unwrap();
//!
//!     loop {
//!         if let Ok(line) = editor.readline(&mut stdin, &mut stdout) {
//!             write!(stdout, "Read: '{}'\n\r", line).unwrap();
//!         } else {
//!             break;
//!         }
//!     }
//! }
//! ```

#![no_std]

#[cfg(any(test, feature = "std"))]
#[macro_use]
extern crate std;

mod common;
pub mod error;
mod input;
pub mod line_buffer;
mod marker;
pub mod no_sync;
mod output;
pub mod sync;
pub(crate) mod terminal;
mod utf8;

#[cfg(test)]
pub(crate) mod testlib;
