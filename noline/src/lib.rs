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
//! - Line history
//!
//! Possible future features:
//! - Auto-completion and hints
//!
//! The API should be considered experimental and will change in the
//! future.
//!
//! The core implementation consists of a state machie taking bytes as
//! input and yielding iterators over byte slices. Because this is
//! done without any IO, Noline can be adapted to work on any platform.
//!
//! Noline comes with multiple implemenations:
//! - [`sync::Editor`] – Editor for asynchronous IO with two separate IO wrappers:
//!   - [`sync::std::IO`] – IO wrapper for [`std::io::Read`] and [`std::io::Write`] traits
//!   - [`sync::embedded::IO`] – IO wrapper for [`embedded_hal::serial::Read`] and [`embedded_hal::serial::Write`]
//! - [`no_sync::tokio::Editor`] - Editor for [`tokio::io::AsyncRead`] and [`tokio::io::AsyncWrite`]
//!
//! Editors can be built using [`builder::EditorBuilder`].
//!
//! # Example
//! ```no_run
//! use noline::{sync::std::IO, builder::EditorBuilder};
//! use std::fmt::Write;
//! use std::io;
//! use termion::raw::IntoRawMode;
//!
//! fn main() {
//!     let stdin = io::stdin();
//!     let stdout = io::stdout().into_raw_mode().unwrap();
//!     let prompt = "> ";
//!
//!     let mut io = IO::new(stdin, stdout);
//!
//!     let mut editor = EditorBuilder::new_unbounded()
//!         .with_unbounded_history()
//!         .build_sync(&mut io)
//!         .unwrap();
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
pub mod builder;
mod core;
pub mod error;
pub mod history;
mod input;
pub mod line_buffer;
pub mod no_sync;
mod output;
pub mod sync;
pub(crate) mod terminal;
mod utf8;

#[cfg(test)]
pub(crate) mod testlib;
