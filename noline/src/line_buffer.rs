//! Buffer to hold line.
//!
//! Can be backed by [`std::vec::Vec<u8>`] for dynamic allocation or
//! [`StaticBuffer`] for static allocation. Custom implementation can
//! be provided with the [`Buffer`] trait.

use crate::utf8::Utf8Char;
use core::{ops::Range, str::from_utf8_unchecked};

/// Trait for defining underlying buffer
pub trait Buffer: Default {
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

/// High level interface to line buffer
pub struct LineBuffer<B: Buffer> {
    buf: B,
}

impl<B: Buffer> LineBuffer<B> {
    /// Create new line buffer
    pub fn new() -> Self {
        Self { buf: B::default() }
    }

    /// Return buffer as bytes slice
    pub fn as_slice(&self) -> &[u8] {
        self.buf.as_slice()
    }

    /// Return buffer length
    pub fn len(&self) -> usize {
        self.buf.buffer_len()
    }

    /// Return buffer as string. The buffer should only hold a valid
    /// UTF-8, so this function is infallible.
    pub fn as_str(&self) -> &str {
        // Pinky swear, it's only UTF-8!
        unsafe { from_utf8_unchecked(self.as_slice()) }
    }

    fn char_ranges<'a>(&'a self) -> impl Iterator<Item = (Range<usize>, char)> + 'a {
        let s = self.as_str();

        s.char_indices()
            .zip(
                s.char_indices()
                    .skip(1)
                    .chain([(s.len(), '\0')].into_iter()),
            )
            .map(|((start, c), (end, _))| (start..end, c))
    }

    fn get_byte_position(&self, char_index: usize) -> usize {
        let s = self.as_str();

        s.char_indices()
            .skip(char_index)
            .map(|(pos, _)| pos)
            .next()
            .unwrap_or(s.len())
    }

    /// Delete character at character index.
    pub fn delete(&mut self, char_index: usize) {
        let mut ranges = self.char_ranges().skip(char_index);

        if let Some((range, _)) = ranges.next() {
            drop(ranges);

            let pos = range.start;

            for _ in range {
                self.buf.remove_byte(pos);
            }
        }
    }

    /// Delete buffer after character index
    pub fn delete_after_char(&mut self, char_index: usize) {
        let pos = self.get_byte_position(char_index);

        self.buf.truncate_buffer(pos);
    }

    /// Truncate buffer
    pub fn truncate(&mut self) {
        self.delete_after_char(0);
    }

    fn delete_range(&mut self, range: Range<usize>) {
        let pos = range.start;
        for _ in range {
            self.buf.remove_byte(pos);
        }
    }

    /// Delete previous word from character index
    pub fn delete_previous_word(&mut self, char_index: usize) -> usize {
        let mut word_start = 0;
        let mut word_end = 0;

        for (i, (range, c)) in self.char_ranges().enumerate().take(char_index) {
            if c == ' ' && i < char_index - 1 {
                word_start = range.end;
            }

            word_end = range.end;
        }

        let deleted = self.as_str()[word_start..word_end].chars().count();

        self.delete_range(word_start..word_end);

        deleted
    }

    /// Swap characters at index
    pub fn swap_chars(&mut self, char_index: usize) {
        let mut ranges = self.char_ranges().skip(char_index - 1);

        if let Some((prev, _)) = ranges.next() {
            if let Some((cur, _)) = ranges.next() {
                drop(ranges);

                for (remove, insert) in cur.zip((prev.start)..) {
                    let byte = self.buf.remove_byte(remove);
                    self.buf.insert_byte(insert, byte);
                }
            }
        }
    }

    /// Insert UTF-8 char at position
    pub fn insert_utf8_char(&mut self, char_index: usize, c: Utf8Char) -> Result<(), Utf8Char> {
        let pos = self.get_byte_position(char_index);

        if let Some(capacity) = self.buf.capacity() {
            if c.as_bytes().len() > capacity - self.buf.buffer_len() {
                return Err(c);
            }
        }

        for (i, byte) in c.as_bytes().iter().enumerate() {
            self.buf.insert_byte(pos + i, *byte);
        }

        Ok(())
    }

    #[cfg(test)]
    pub fn insert_str(&mut self, char_index: usize, s: &str) {
        use ::std::string::ToString;

        for (pos, c) in s
            .chars()
            .map(|c| Utf8Char::from_str(&c.to_string()))
            .enumerate()
        {
            assert!(self.insert_utf8_char(char_index + pos, c).is_ok());
        }
    }
}

/// Static buffer backed by array
pub struct StaticBuffer<const N: usize> {
    array: [u8; N],
    len: usize,
}

impl<const N: usize> StaticBuffer<N> {
    pub fn new() -> Self {
        Self {
            array: [0; N],
            len: 0,
        }
    }
}

impl<const N: usize> Default for StaticBuffer<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> Buffer for StaticBuffer<N> {
    fn buffer_len(&self) -> usize {
        self.len
    }

    fn capacity(&self) -> Option<usize> {
        Some(N)
    }

    fn truncate_buffer(&mut self, index: usize) {
        self.len = index;
    }

    fn insert_byte(&mut self, index: usize, byte: u8) {
        for i in (index..self.len).rev() {
            self.array[i + 1] = self.array[i];
        }

        self.array[index] = byte;
        self.len += 1;
    }

    fn remove_byte(&mut self, index: usize) -> u8 {
        let byte = self.array[index];

        for i in index..(self.len - 1) {
            self.array[i] = self.array[i + 1];
        }

        self.len -= 1;

        byte
    }

    fn as_slice(&self) -> &[u8] {
        &self.array[0..self.len]
    }
}

#[cfg(any(test, feature = "alloc", feature = "std"))]
mod alloc {
    extern crate alloc;

    use self::alloc::vec::Vec;
    use super::*;

    impl Buffer for Vec<u8> {
        fn buffer_len(&self) -> usize {
            self.len()
        }

        fn capacity(&self) -> Option<usize> {
            None
        }

        fn truncate_buffer(&mut self, index: usize) {
            self.truncate(index)
        }

        fn insert_byte(&mut self, index: usize, byte: u8) {
            self.insert(index, byte);
        }

        fn remove_byte(&mut self, index: usize) -> u8 {
            self.remove(index)
        }

        fn as_slice(&self) -> &[u8] {
            self.as_slice()
        }
    }
}

#[cfg(any(test, feature = "alloc", feature = "std"))]
pub use self::alloc::*;

#[cfg(test)]
mod tests {
    use std::vec::Vec;

    use super::*;

    #[test]
    fn static_buffer() {
        let mut buf: StaticBuffer<20> = StaticBuffer::new();

        for i in 0..20 {
            buf.insert_byte(i, 0x30);
        }

        buf.remove_byte(19);
    }

    fn insert_str<B: Buffer>(buf: &mut LineBuffer<B>, index: usize, s: &str) {
        buf.insert_str(index, s);
    }

    fn test_line_buffer<B: Buffer>(buf: &mut LineBuffer<B>) {
        insert_str(buf, 0, "Hello, World!");

        assert_eq!(buf.as_str(), "Hello, World!");

        buf.delete(12);

        assert_eq!(buf.as_str(), "Hello, World");

        buf.delete(12);

        assert_eq!(buf.as_str(), "Hello, World");

        buf.delete(0);
        insert_str(buf, 0, "h");

        assert_eq!(buf.as_str(), "hello, World");

        buf.delete(2);
        insert_str(buf, 2, "L");

        assert_eq!(buf.as_str(), "heLlo, World");

        buf.delete(11);

        assert_eq!(buf.as_str(), "heLlo, Worl");

        buf.delete(5);

        assert_eq!(buf.as_str(), "heLlo Worl");

        for _ in 0..5 {
            buf.delete(5);
        }

        assert_eq!(buf.as_str(), "heLlo");

        insert_str(buf, 5, " æå");

        assert_eq!(buf.as_str(), "heLlo æå");

        insert_str(buf, 7, "ø");

        assert_eq!(buf.as_str(), "heLlo æøå");

        buf.delete(8);

        assert_eq!(buf.as_str(), "heLlo æø");

        buf.delete(7);

        assert_eq!(buf.as_str(), "heLlo æ");

        buf.delete_previous_word(7);

        assert_eq!(buf.as_str(), "heLlo ");

        buf.delete_previous_word(6);

        assert_eq!(buf.as_str(), "");

        insert_str(buf, 0, "word1 word2 word3");
        assert_eq!(buf.as_str(), "word1 word2 word3");
        buf.delete_previous_word(12);

        assert_eq!(buf.as_str(), "word1 word3");
    }

    #[test]
    fn test_static_line_buffer() {
        let mut buf = LineBuffer::<StaticBuffer<80>>::new();

        test_line_buffer(&mut buf);

        buf.delete_after_char(0);

        assert_eq!(buf.len(), 0);

        for i in 0..80 {
            assert!(buf.insert_utf8_char(i, Utf8Char::from_str("a")).is_ok());
        }

        assert!(buf.insert_utf8_char(80, Utf8Char::from_str("a")).is_err());
    }

    #[test]
    fn test_alloc_line_buffer() {
        let mut buf = LineBuffer::<Vec<u8>>::new();

        test_line_buffer(&mut buf);

        buf.delete_after_char(0);

        for i in 0..1000 {
            assert!(buf.insert_utf8_char(i, Utf8Char::from_str("a")).is_ok());
        }
    }
}
