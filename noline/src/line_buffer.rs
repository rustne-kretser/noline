use core::{ops::Range, str::from_utf8_unchecked};

use staticvec::StaticVec;

pub trait Buffer: Default {
    fn buffer_len(&self) -> usize;
    fn capacity(&self) -> Option<usize>;
    fn truncate_buffer(&mut self, index: usize);
    fn insert_byte(&mut self, index: usize, byte: u8);
    fn remove_byte(&mut self, index: usize) -> u8;
    fn as_slice(&self) -> &[u8];
}

impl<const N: usize> Buffer for StaticVec<u8, N> {
    fn buffer_len(&self) -> usize {
        self.len()
    }

    fn capacity(&self) -> Option<usize> {
        Some(N)
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

pub struct LineBuffer<B: Buffer> {
    buf: B,
}

impl<B: Buffer> LineBuffer<B> {
    pub fn new() -> Self {
        Self { buf: B::default() }
    }

    pub fn as_slice(&self) -> &[u8] {
        self.buf.as_slice()
    }

    pub fn len(&self) -> usize {
        self.buf.buffer_len()
    }

    pub fn as_str(&self) -> &str {
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

    pub fn delete_after_char(&mut self, char_index: usize) {
        let pos = self.get_byte_position(char_index);

        self.buf.truncate_buffer(pos);
    }

    fn delete_range(&mut self, range: Range<usize>) {
        let pos = range.start;
        for _ in range {
            self.buf.remove_byte(pos);
        }
    }

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
        use std::string::ToString;

        for (pos, c) in s
            .chars()
            .map(|c| Utf8Char::from_str(&c.to_string()))
            .enumerate()
        {
            assert!(self.insert_utf8_char(char_index + pos, c).is_ok());
        }
    }
}

pub type StaticLineBuffer<const N: usize> = LineBuffer<StaticVec<u8, N>>;

#[cfg(any(test, feature = "std"))]
mod feature_std {
    use super::*;
    use std::vec::Vec;

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

    pub type AllocLineBuffer = LineBuffer<Vec<u8>>;
}

#[cfg(any(test, feature = "std"))]
pub use feature_std::*;

use crate::utf8::Utf8Char;

#[cfg(test)]
mod tests {
    use super::*;

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
        let mut buf = StaticLineBuffer::<80>::new();

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
        let mut buf = AllocLineBuffer::new();

        test_line_buffer(&mut buf);

        buf.delete_after_char(0);

        for i in 0..1000 {
            assert!(buf.insert_utf8_char(i, Utf8Char::from_str("a")).is_ok());
        }
    }
}
