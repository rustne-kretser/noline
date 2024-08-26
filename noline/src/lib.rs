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
//! The core implementation consists of a state machine taking bytes as
//! input and yielding iterators over byte slices. Because this is
//! done without any IO, Noline can be adapted to work on any platform.
//!
//! Noline comes with multiple implemenations:
//! - [`sync_editor::Editor`] – Editor for synchronous IO
//! - [`async_editor::Editor`] - Editor for asynchronous IO
//!
//! Editors can be built using [`builder::EditorBuilder`].
//!
//! # Example
//! ```no_run
//! # use noline::{builder::EditorBuilder};
//! # use embedded_io::{Read, Write, ErrorType};
//! # use core::convert::Infallible;
//! # struct MyIO {}
//! # impl ErrorType for MyIO {
//! #     type Error = Infallible;
//! # }
//! # impl embedded_io::Write for MyIO {
//! #     fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> { unimplemented!() }
//! #     fn flush(&mut self) -> Result<(), Self::Error> { unimplemented!() }
//! # }
//! # impl embedded_io::Read for MyIO {
//! #     fn read(&mut self, buf: &mut[u8]) -> Result<usize, Self::Error> { unimplemented!() }
//! # }
//! # let mut io = MyIO {};
//! let prompt = "> ";
//!
//! let mut io = MyIO {}; // IO handler, see full examples for details
//!                       // how to implement it
//!
//! let mut editor = EditorBuilder::new_unbounded()
//!     .with_unbounded_history()
//!     .build_sync(&mut io)
//!     .unwrap();
//!
//! while let Ok(line) = editor.readline(prompt, &mut io) {
//!     writeln!(io, "Read: '{}'", line).unwrap();
//! }
//! ```

#![cfg_attr(not(test), no_std)]

pub mod async_editor;
pub mod builder;
mod core;
pub mod error;
pub mod history;
mod input;
pub mod line_buffer;
mod output;
pub mod sync_editor;
pub(crate) mod terminal;
mod utf8;

#[cfg(test)]
pub(crate) mod testlib;
