use super::Buffer;

/// Static buffer backed by slice
pub struct SliceBuffer<'a> {
    data: &'a mut [u8],
    len: usize,
}

impl<'a> SliceBuffer<'a> {
    pub fn new(data: &'a mut [u8]) -> Self {
        Self { data, len: 0 }
    }
}

impl<'a> Buffer for SliceBuffer<'a> {
    fn buffer_len(&self) -> usize {
        self.len
    }

    fn capacity(&self) -> Option<usize> {
        Some(self.data.len())
    }

    fn truncate_buffer(&mut self, index: usize) {
        self.len = index;
    }

    fn insert_byte(&mut self, index: usize, byte: u8) {
        for i in (index..self.len).rev() {
            self.data[i + 1] = self.data[i];
        }

        self.data[index] = byte;
        self.len += 1;
    }

    fn remove_byte(&mut self, index: usize) -> u8 {
        let byte = self.data[index];

        for i in index..(self.len - 1) {
            self.data[i] = self.data[i + 1];
        }

        self.len -= 1;

        byte
    }

    fn as_slice(&self) -> &[u8] {
        &self.data[0..self.len]
    }
}
