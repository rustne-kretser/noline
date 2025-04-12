//! Core library containing the components for building an editor.
//!
//! Use [`Initializer`] to get [`crate::terminal::Terminal`] and then
//! use [`Line`] to read a single line.

mod line;
mod prompt;
mod reset_handle;
mod str_iter;

pub use line::*;
pub use prompt::*;
pub use reset_handle::*;
pub use str_iter::*;

#[cfg(test)]
pub(crate) mod tests;
