//! Noline is an IO-agnostic `#[no_std]` line editor providing robust
//! line editing for any system. The core functionality is IO-free, so
//! it can be adapted to any system be it embedded, async, async
//! embedded, WASM or IPoAC (IP over Avian Carriers).
//!
//! The core consists of a massive state machine taking bytes as input
//! and returning an iterator over byte slices. There are, however,
//! some convenince wrappers:
//! - Sync std
//! - Sync embedded
//! - Async tokio

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
