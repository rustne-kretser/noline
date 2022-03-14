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
//! - [`sync::Editor`]
//!   - [`sync::std::IO`]
//!   - [`sync::embedded::IO`]
//! - [`no_sync::tokio::Editor`]
//!
//! # Feature flags
//!
//! All features are enabled by default
//!
//! # Example
//! ```no_run
//! use noline::sync::{std::IO, Editor};
//! use std::io;
//! use std::fmt::Write;
//! use termion::raw::IntoRawMode;
//!
//! fn main() {
//!     let mut stdin = io::stdin();
//!     let mut stdout = io::stdout().into_raw_mode().unwrap();
//!     let prompt = "> ";
//!
//!     let mut io = IO::new(stdin, stdout);
//!     let mut editor = Editor::<Vec<u8>, _>::new(&mut io).unwrap();
//!
//!     loop {
//!         if let Ok(line) = editor.readline(prompt, &mut io) {
//!             write!(io, "Read: '{}'\n\r", line).unwrap();
//!         } else {
//!             break;
//!         }
//!     }
//! }
//! ```

#![no_std]

#[cfg(any(test, doc, feature = "std"))]
#[macro_use]
extern crate std;

mod core;
pub mod error;
mod input;
pub mod line_buffer;
pub mod no_sync;
mod output;
pub mod sync;
pub(crate) mod terminal;
mod utf8;

#[cfg(test)]
pub(crate) mod testlib;
