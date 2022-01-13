use core::ops::Range;

use staticvec::StaticVec;

#[derive(Copy, Clone)]
pub enum CursorMove {
    Back,
    Forward,
    Start,
    End,
}

trait Utf8 {
    fn is_start(&self) -> bool;

    fn is_continuation(&self) -> bool;
}

impl Utf8 for u8 {
    fn is_start(&self) -> bool {
        let byte = *self;

        byte & 0b10000000 == 0
            || byte & 0b11100000 == 0b11000000
            || byte & 0b11110000 == 0b11100000
            || byte & 0b11111000 == 0b11110000
    }

    fn is_continuation(&self) -> bool {
        *self & 0b11000000 == 0b10000000
    }
}

pub trait LineBuffer {
    fn update_position(&mut self, new: usize);
    fn get_position(&self) -> usize;
    fn buffer_len(&self) -> usize;
    fn buffer_capacity(&self) -> Option<usize>;
    fn as_slice(&self) -> &[u8];
    fn as_mut_slice(&mut self) -> &mut [u8];
    fn delete_byte(&mut self, pos: usize) -> u8;
    fn delete_after(&mut self);
    fn insert_in_buffer(&mut self, pos: usize, byte: u8) -> Result<(), u8>;

    fn prev_byte(&self, pos: usize) -> Option<u8> {
        if pos > 0 {
            self.get_byte(pos - 1)
        } else {
            None
        }
    }

    fn delete_previous_word(&mut self) -> Result<(), ()> {
        let mut pos = self.get_position();

        if let Some(prev) = self.prev_byte(pos) {
            if prev == (' ' as u8) {
                pos -= 1;
                self.delete_byte(pos);
            }

            while let Some(prev) = self.prev_byte(pos) {
                if prev == (' ' as u8) {
                    break;
                }

                pos -= 1;

                self.delete_byte(pos);
            }

            self.update_position(pos);
            return Ok(());
        }

        Err(())
    }

    fn swap_previous(&mut self) {
        let after = self.char_after();
        let before = self.char_before();

        if before.len() + after.len() > 0 {
            let mut pos = before.start;

            for i in after {
                let byte = self.delete_byte(i);

                self.insert_in_buffer(pos, byte).unwrap();
                pos += 1;
            }

            self.update_position(pos);
        }
    }

    fn char_after(&self) -> Range<usize> {
        let mut pos = self.get_position();
        let start = pos;

        if self.get_byte(pos).is_none() {
            return start..start;
        }

        let end = loop {
            pos += 1;

            if let Some(byte) = self.get_byte(pos) {
                if byte.is_start() {
                    break pos;
                }
            } else {
                break pos;
            }
        };

        start..end
    }

    fn char_before(&self) -> Range<usize> {
        let mut pos = self.get_position();
        let end = pos;

        let start = loop {
            if pos == 0 {
                break 0;
            }

            pos -= 1;

            if let Some(byte) = self.get_byte(pos) {
                if byte.is_start() {
                    break pos;
                }
            } else {
                break pos;
            }
        };

        start..end
    }

    fn get_byte(&self, pos: usize) -> Option<u8> {
        let buffer = self.as_slice();

        if pos < buffer.len() {
            Some(buffer[pos])
        } else {
            None
        }
    }

    fn current_byte(&self) -> Option<u8> {
        self.get_byte(self.get_position())
    }

    fn bytes_after(&self, pos: usize) -> &[u8] {
        &self.as_slice()[pos..]
    }

    fn insert_byte(&mut self, byte: u8) -> Result<(), u8> {
        let pos = self.get_position();
        self.insert_in_buffer(pos, byte)?;
        self.update_position(pos + 1);

        Ok(())
    }

    fn insert_bytes<'a>(&mut self, bytes: &'a [u8]) -> Result<(), &'a [u8]> {
        if let Some(capacity) = self.buffer_capacity() {
            if bytes.len() > capacity - self.buffer_len() {
                return Err(bytes);
            }
        }

        for b in bytes {
            self.insert_byte(*b).unwrap();
        }

        Ok(())
    }

    fn delete(&mut self) -> Result<(), ()> {
        let range = self.char_after();

        if range.len() > 0 {
            for i in range.rev() {
                self.delete_byte(i);
            }
            Ok(())
        } else {
            Err(())
        }
    }

    fn backspace(&mut self) -> Result<(), ()> {
        if self.move_cursor(CursorMove::Back).is_ok() {
            self.delete()
        } else {
            Err(())
        }
    }

    fn move_cursor(&mut self, cursor_move: CursorMove) -> Result<(), ()> {
        let pos = self.get_position();

        let new = match cursor_move {
            CursorMove::Back => self.char_before().start,
            CursorMove::Forward => self.char_after().end,
            CursorMove::Start => 0,
            CursorMove::End => self.buffer_len(),
        };

        if new != pos {
            self.update_position(new);
            Ok(())
        } else {
            Err(())
        }
    }
}

pub struct StaticLineBuffer<const N: usize> {
    buffer: StaticVec<u8, N>,
    cursor: usize,
}

impl<const N: usize> StaticLineBuffer<N> {
    pub fn new() -> Self {
        Self {
            buffer: StaticVec::new(),
            cursor: 0,
        }
    }
}

impl<const N: usize> LineBuffer for StaticLineBuffer<N> {
    fn as_slice(&self) -> &[u8] {
        self.buffer.as_slice()
    }

    fn as_mut_slice(&mut self) -> &mut [u8] {
        self.buffer.as_mut_slice()
    }

    fn delete_byte(&mut self, pos: usize) -> u8 {
        let byte = self.buffer[pos];
        self.buffer.remove(pos);
        byte
    }

    fn delete_after(&mut self) {
        self.buffer.truncate(self.get_position());
    }

    fn insert_in_buffer(&mut self, pos: usize, byte: u8) -> Result<(), u8> {
        if pos < N {
            self.buffer.insert(pos, byte);
            Ok(())
        } else {
            Err(byte)
        }
    }

    fn update_position(&mut self, new: usize) {
        self.cursor = new;
    }

    fn get_position(&self) -> usize {
        self.cursor
    }

    fn buffer_len(&self) -> usize {
        self.buffer.len()
    }

    fn buffer_capacity(&self) -> Option<usize> {
        Some(N)
    }
}

#[cfg(any(test, feature = "std"))]
mod feature_std {
    use super::*;
    use std::vec::Vec;

    pub struct AllocLineBuffer {
        buffer: Vec<u8>,
        cursor: usize,
    }

    impl AllocLineBuffer {
        pub fn new() -> Self {
            Self {
                buffer: Vec::new(),
                cursor: 0,
            }
        }
    }

    impl LineBuffer for AllocLineBuffer {
        fn as_slice(&self) -> &[u8] {
            self.buffer.as_slice()
        }

        fn as_mut_slice(&mut self) -> &mut [u8] {
            self.buffer.as_mut_slice()
        }

        fn delete_byte(&mut self, pos: usize) -> u8 {
            let byte = self.buffer[pos];
            self.buffer.remove(pos);
            byte
        }

        fn delete_after(&mut self) {
            self.buffer.truncate(self.get_position());
        }

        fn insert_in_buffer(&mut self, pos: usize, byte: u8) -> Result<(), u8> {
            self.buffer.insert(pos, byte);
            Ok(())
        }

        fn update_position(&mut self, new: usize) {
            self.cursor = new;
        }

        fn get_position(&self) -> usize {
            self.cursor
        }

        fn buffer_len(&self) -> usize {
            self.buffer.len()
        }

        fn buffer_capacity(&self) -> Option<usize> {
            None
        }
    }
}

#[cfg(any(test, feature = "std"))]
pub use feature_std::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf_8() {
        let bytes = "æ".as_bytes();

        assert_eq!(bytes.len(), 2);
        assert!(bytes[0].is_start());
        assert!(!bytes[0].is_continuation());
        assert!(bytes[1].is_continuation());
        assert!(!bytes[1].is_start());
    }

    fn test_line_buffer<LB: LineBuffer>(buf: &mut LB) {
        for byte in "Hello, World!".as_bytes() {
            buf.insert_byte(*byte).unwrap();
        }

        assert_eq!("Hello, World!".as_bytes(), buf.as_slice());

        assert!(buf.backspace().is_ok());

        assert_eq!("Hello, World".as_bytes(), buf.as_slice());

        assert!(buf.delete().is_err());

        for _ in 0..7 {
            assert!(buf.move_cursor(CursorMove::Back).is_ok());
        }

        assert!(buf.delete().is_ok());

        assert_eq!("Hello World".as_bytes(), buf.as_slice());

        assert!(buf.move_cursor(CursorMove::Start).is_ok());
        assert!(buf.move_cursor(CursorMove::Back).is_err());
        assert!(buf.move_cursor(CursorMove::Start).is_err());

        assert!(buf.delete().is_ok());

        buf.insert_byte('h' as u8).unwrap();

        assert_eq!("hello World".as_bytes(), buf.as_slice());

        assert!(buf.move_cursor(CursorMove::Forward).is_ok());
        buf.insert_byte('L' as u8).unwrap();
        assert!(buf.delete().is_ok());

        assert_eq!("heLlo World".as_bytes(), buf.as_slice());

        assert!(buf.move_cursor(CursorMove::End).is_ok());
        assert!(buf.delete().is_err());
        assert_eq!("heLlo World".as_bytes(), buf.as_slice());

        assert!(buf.backspace().is_ok());

        assert_eq!("heLlo Worl".as_bytes(), buf.as_slice());

        for _ in 0..5 {
            assert!(buf.move_cursor(CursorMove::Back).is_ok());
        }

        buf.delete_after();

        assert_eq!("heLlo".as_bytes(), buf.as_slice());

        buf.insert_byte(' ' as u8).unwrap();

        for b in "æå".as_bytes() {
            buf.insert_byte(*b).unwrap();
        }

        assert_eq!("heLlo æå".as_bytes(), buf.as_slice());

        assert!(buf.move_cursor(CursorMove::Back).is_ok());

        for b in "ø".as_bytes() {
            buf.insert_byte(*b).unwrap();
        }

        assert_eq!("heLlo æøå".as_bytes(), buf.as_slice());

        assert!(buf.delete().is_ok());

        assert_eq!("heLlo æø".as_bytes(), buf.as_slice());

        assert!(buf.backspace().is_ok());

        assert_eq!("heLlo æ".as_bytes(), buf.as_slice());

        assert!(buf.delete_previous_word().is_ok());

        assert_eq!("heLlo ".as_bytes(), buf.as_slice());

        assert!(buf.delete_previous_word().is_ok());

        assert_eq!("".as_bytes(), buf.as_slice());

        assert!(buf.delete_previous_word().is_err());
    }

    #[test]
    fn static_line_buffer() {
        let mut buf: StaticLineBuffer<80> = StaticLineBuffer::new();

        test_line_buffer(&mut buf);

        while buf.backspace().is_ok() {}

        for _ in 0..80 {
            assert!(buf.insert_byte('a' as u8).is_ok());
        }

        assert!(buf.insert_byte('a' as u8).is_err());
    }

    #[test]
    fn alloc_line_buffer() {
        let mut buf = AllocLineBuffer::new();

        test_line_buffer(&mut buf);

        for _ in 0..1000 {
            assert!(buf.insert_byte('a' as u8).is_ok());
        }
    }
}
