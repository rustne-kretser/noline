use super::{Buffer, SliceBuffer};
use crate::utf8::Utf8Char;
use core::{ops::Range, str::from_utf8_unchecked};

/// High level interface to line buffer
pub struct LineBuffer<B: Buffer> {
    pub(super) buf: B,
}

impl<'a> LineBuffer<SliceBuffer<'a>> {
    /// Create new static line buffer
    pub fn from_slice(buffer: &'a mut [u8]) -> Self {
        Self {
            buf: SliceBuffer::new(buffer),
        }
    }
}

impl<B: Buffer> LineBuffer<B> {
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

    fn char_ranges(&self) -> impl Iterator<Item = (Range<usize>, char)> + '_ {
        let s = self.as_str();

        s.char_indices()
            .zip(s.char_indices().skip(1).chain([(s.len(), '\0')]))
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

    /// Insert bytes at index
    ///
    /// # Safety
    ///
    /// The caller must ensure that the input bytes are a valid UTF-8
    /// sequence and that the byte index aligns with a valid UTF-8 character index.
    pub unsafe fn insert_bytes(&mut self, index: usize, bytes: &[u8]) -> Result<(), ()> {
        if let Some(capacity) = self.buf.capacity() {
            if bytes.len() > capacity - self.buf.buffer_len() {
                return Err(());
            }
        }

        for (i, byte) in bytes.iter().enumerate() {
            self.buf.insert_byte(index + i, *byte);
        }

        Ok(())
    }

    /// Insert UTF-8 char at position
    pub fn insert_utf8_char(&mut self, char_index: usize, c: Utf8Char) -> Result<(), Utf8Char> {
        unsafe {
            self.insert_bytes(self.get_byte_position(char_index), c.as_bytes())
                .map_err(|_| c)
        }
    }

    /// Insert string at char position
    pub fn insert_str(&mut self, char_index: usize, s: &str) -> Result<(), ()> {
        unsafe { self.insert_bytes(self.get_byte_position(char_index), s.as_bytes()) }
    }
}
