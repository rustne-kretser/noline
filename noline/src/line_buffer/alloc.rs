extern crate alloc;

use self::alloc::vec::Vec;
use super::*;

impl LineBuffer<UnboundedBuffer> {
    /// Create new static line buffer
    pub fn new_unbounded() -> Self {
        Self {
            buf: UnboundedBuffer::new(),
        }
    }
}

pub struct UnboundedBuffer {
    vec: Vec<u8>,
}

impl UnboundedBuffer {
    pub fn new() -> Self {
        Self { vec: Vec::new() }
    }
}

impl Buffer for UnboundedBuffer {
    fn buffer_len(&self) -> usize {
        self.vec.len()
    }

    fn capacity(&self) -> Option<usize> {
        None
    }

    fn truncate_buffer(&mut self, index: usize) {
        self.vec.truncate(index)
    }

    fn insert_byte(&mut self, index: usize, byte: u8) {
        self.vec.insert(index, byte);
    }

    fn remove_byte(&mut self, index: usize) -> u8 {
        self.vec.remove(index)
    }

    fn as_slice(&self) -> &[u8] {
        self.vec.as_slice()
    }
}
