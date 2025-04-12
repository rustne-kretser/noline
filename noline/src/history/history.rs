use super::CircularSlice;

/// Trait for line history
pub trait History {
    /// Return entry at index, or None if out of bounds
    fn get_entry(&self, index: usize) -> Option<CircularSlice<'_>>;

    /// Add new entry at the end
    fn add_entry<'a>(&mut self, entry: &'a str) -> Result<(), &'a str>;

    /// Return number of entries in history
    fn number_of_entries(&self) -> usize;

    /// Add entries from an iterator
    fn load_entries<'a, I: Iterator<Item = &'a str>>(&mut self, entries: I) -> usize {
        entries
            .take_while(|entry| self.add_entry(entry).is_ok())
            .count()
    }
}

/// Return an iterator over history entries
///
/// # Note
///
/// This should ideally be in the [`History`] trait, but is
/// until `type_alias_impl_trait` is stable.
pub(crate) fn get_history_entries<H: History>(
    history: &H,
) -> impl Iterator<Item = CircularSlice<'_>> {
    (0..(history.number_of_entries())).filter_map(|index| history.get_entry(index))
}
