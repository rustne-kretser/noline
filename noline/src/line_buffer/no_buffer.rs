use super::Buffer;

/// Emtpy buffer used for builder
pub struct NoBuffer {}

impl Buffer for NoBuffer {
    fn buffer_len(&self) -> usize {
        unimplemented!()
    }

    fn capacity(&self) -> Option<usize> {
        unimplemented!()
    }

    fn truncate_buffer(&mut self, _index: usize) {
        unimplemented!()
    }

    fn insert_byte(&mut self, _index: usize, _byte: u8) {
        unimplemented!()
    }

    fn remove_byte(&mut self, _index: usize) -> u8 {
        unimplemented!()
    }

    fn as_slice(&self) -> &[u8] {
        unimplemented!()
    }
}
