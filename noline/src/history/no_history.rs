use super::{CircularSlice, History};

/// Emtpy implementation for Editors with no history
pub struct NoHistory {}

impl NoHistory {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for NoHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl History for NoHistory {
    fn get_entry(&self, _index: usize) -> Option<CircularSlice<'_>> {
        None
    }

    fn add_entry<'a>(&mut self, entry: &'a str) -> Result<(), &'a str> {
        Err(entry)
    }

    fn number_of_entries(&self) -> usize {
        0
    }
}
