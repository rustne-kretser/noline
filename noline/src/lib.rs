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
//! - [`sync_editor::Editor`] – Editor for synchronous IO with the following wrapper:
//!   - [`sync_io::IO`] – IO wrapper for [`embedded_io::Read`] and [`embedded_io::Write`]
//! - [`async_editor::Editor`] - Editor for asynchronous IO with the following wrapper:
//!   - [`async_io::IO`] – IO wrapper for [`embedded_io_async::Read`] and [`embedded_io_async::Write`]
//!
//!
//! Editors can be built using [`builder::EditorBuilder`].
//!
//! # Example
//! ```no_run
//! use noline::{builder::EditorBuilder, sync_io::std_sync::StdIOWrapper, sync_io::IO};
//! use std::fmt::Write;
//! use std::io;
//! use termion::raw::IntoRawMode;
//!
//! fn main() {
//!     let _stdout = io::stdout().into_raw_mode().unwrap();
//!     let prompt = "> ";
//!
//!     let mut io = IO::<StdIOWrapper>::new(StdIOWrapper::new());
//!
//!     let mut editor = EditorBuilder::new_unbounded()
//!         .with_unbounded_history()
//!         .build_sync(&mut io)
//!         .unwrap();
//!
//!     while let Ok(line) = editor.readline(prompt, &mut io) {
//!         writeln!(io, "Read: '{}'", line).unwrap();
//!     }
//! }
//! ```

#![no_std]

#[cfg(any(test, doc, feature = "std"))]
#[macro_use]
extern crate std;
#[cfg(any(test, doc, feature = "async"))]
pub mod async_editor;
#[cfg(any(test, doc, feature = "async"))]
pub mod async_io;
pub mod builder;
mod core;
pub mod error;
pub mod history;
mod input;
pub mod line_buffer;
mod output;
#[cfg(any(test, doc, feature = "sync"))]
pub mod sync_editor;
#[cfg(any(test, doc, feature = "sync"))]
pub mod sync_io;
pub(crate) mod terminal;
mod utf8;

#[cfg(test)]
pub(crate) mod testlib;
