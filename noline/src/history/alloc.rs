use super::*;
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

extern crate alloc;

/// Unbounded history backed by [`Vec<String>`]
pub struct UnboundedHistory {
    buffer: Vec<String>,
}

impl UnboundedHistory {
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }
}

impl Default for UnboundedHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl History for UnboundedHistory {
    fn get_entry(&self, index: usize) -> Option<CircularSlice<'_>> {
        let s = self.buffer[index].as_str();

        Some(CircularSlice::new(s.as_bytes(), 0, s.len(), s.len()))
    }

    fn add_entry<'a>(&mut self, entry: &'a str) -> Result<(), &'a str> {
        self.buffer.push(entry.to_string());

        #[cfg(test)]
        dbg!(entry);

        Ok(())
    }

    fn number_of_entries(&self) -> usize {
        self.buffer.len()
    }
}
