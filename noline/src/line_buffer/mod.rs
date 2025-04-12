//! Buffer to hold line.
//!
//! Can be backed by [`std::vec::Vec<u8>`] for dynamic allocation or
//! [`StaticBuffer`] for static allocation. Custom implementation can
//! be provided with the [`Buffer`] trait.

mod buffer;
mod line_buffer;
mod no_buffer;
mod slice_buffer;

#[cfg(any(test, doc, feature = "alloc", feature = "std"))]
mod alloc;

pub use buffer::*;
pub use line_buffer::*;
pub use no_buffer::*;
pub use slice_buffer::*;

#[cfg(any(test, doc, feature = "alloc", feature = "std"))]
pub use self::alloc::*;

#[cfg(test)]
pub(crate) mod tests;
