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
