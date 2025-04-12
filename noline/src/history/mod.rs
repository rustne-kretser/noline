//! Line history

mod circ_index;
mod circ_range;
mod circ_slice;
mod history;
mod history_nav;
mod no_history;
mod slice_history;
mod window;

pub use circ_slice::*;
pub use history::*;
pub(super) use history_nav::HistoryNavigator;
pub use no_history::*;
pub use slice_history::*;

#[cfg(any(test, doc, feature = "alloc", feature = "std"))]
mod alloc;

#[cfg(any(test, doc, feature = "alloc", feature = "std"))]
pub use alloc::UnboundedHistory;

#[cfg(test)]
pub(crate) mod tests;
