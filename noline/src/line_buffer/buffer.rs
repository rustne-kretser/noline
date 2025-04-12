/// Trait for defining underlying buffer
pub trait Buffer {
    /// Return the current length of the buffer. This represents the
    /// number of bytes currently in the buffer, not the capacity.
    fn buffer_len(&self) -> usize;

    /// Return buffer capacity or None if unbounded.
    fn capacity(&self) -> Option<usize>;

    /// Truncate buffer, setting lenght to 0.
    fn truncate_buffer(&mut self, index: usize);

    /// Insert byte at index
    fn insert_byte(&mut self, index: usize, byte: u8);

    /// Remove byte from index and return byte
    fn remove_byte(&mut self, index: usize) -> u8;

    /// Return byte slice into buffer from 0 up to buffer length.
    fn as_slice(&self) -> &[u8];
}
