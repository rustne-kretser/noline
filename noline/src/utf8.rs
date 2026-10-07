enum Utf8ByteType {
    SingleByte,
    StartTwoByte,
    StartThreeByte,
    StartFourByte,
    Continuation,
    Invalid,
}

trait Utf8Byte {
    fn utf8_byte_type(&self) -> Utf8ByteType;
    fn utf8_is_continuation(&self) -> bool;
}

impl Utf8Byte for u8 {
    fn utf8_byte_type(&self) -> Utf8ByteType {
        let byte = *self;

        if byte & 0b10000000 == 0 {
            Utf8ByteType::SingleByte
        } else if byte & 0b11000000 == 0b10000000 {
            Utf8ByteType::Continuation
        } else if (0xc2..=0xdf).contains(&byte) {
            Utf8ByteType::StartTwoByte
        } else if byte & 0b11110000 == 0b11100000 {
            Utf8ByteType::StartThreeByte
        } else if (0xf0..=0xf4).contains(&byte) {
            Utf8ByteType::StartFourByte
        } else {
            Utf8ByteType::Invalid
        }
    }

    fn utf8_is_continuation(&self) -> bool {
        matches!(self.utf8_byte_type(), Utf8ByteType::Continuation)
    }
}

#[derive(Debug, Eq, PartialEq)]
enum Utf8DecoderState {
    New,
    ExpectingOneByte,
    ExpectingTwoBytes,
    ExpectingThreeBytes,
    Done,
}

#[derive(Eq, PartialEq, Copy, Clone)]
pub struct Utf8Char {
    buf: [u8; 4],
    len: u8,
}

#[cfg(test)]
impl std::fmt::Debug for Utf8Char {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Utf8Char").field(&self.as_char()).finish()
    }
}

impl Utf8Char {
    fn new(bytes: &[u8; 4], len: usize) -> Self {
        Self {
            len: len as u8,
            buf: *bytes,
        }
    }

    #[cfg(test)]
    pub(crate) fn from_str(s: &str) -> Self {
        let bytes = s.as_bytes();
        assert!(bytes.len() <= 4);

        let mut c = Self {
            len: bytes.len() as u8,
            buf: [0; 4],
        };

        for (i, b) in bytes.iter().enumerate() {
            c.buf[i] = *b;
        }

        c
    }

    #[cfg(test)]
    pub(crate) fn as_char(&self) -> char {
        char::from_u32(
            self.as_bytes()
                .iter()
                .fold(0, |codepoint, &b| match b.utf8_byte_type() {
                    Utf8ByteType::SingleByte => b as u32,
                    Utf8ByteType::StartTwoByte => (b & 0x1f) as u32,
                    Utf8ByteType::StartThreeByte => (b & 0xf) as u32,
                    Utf8ByteType::StartFourByte => (b & 0x7) as u32,
                    Utf8ByteType::Continuation => (codepoint << 6) | (b & 0x3f) as u32,
                    Utf8ByteType::Invalid => unreachable!(),
                }),
        )
        .unwrap()
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.buf[0..(self.len as usize)]
    }
}

#[cfg_attr(test, derive(Debug))]
#[derive(Eq, PartialEq)]
pub enum Utf8DecoderStatus {
    Continuation,
    Done(Utf8Char),
    Error,
}

#[derive(Debug, Eq, PartialEq)]
pub struct Utf8Decoder {
    state: Utf8DecoderState,
    buf: [u8; 4],
    pos: usize,
}

impl Utf8Decoder {
    pub fn new() -> Self {
        Self {
            state: Utf8DecoderState::New,
            buf: [0, 0, 0, 0],
            pos: 0,
        }
    }

    fn insert_byte(&mut self, byte: u8) -> Result<(), ()> {
        if self.pos > 0 && !byte.utf8_is_continuation() {
            return Err(());
        }
        // Bounds on the second byte exclude overlong encodings, surrogates,
        // and code points above U+10FFFF.
        if self.pos == 1
            && match self.buf[0] {
                0xe0 => byte < 0xa0,
                0xed => byte > 0x9f,
                0xf0 => byte < 0x90,
                0xf4 => byte > 0x8f,
                _ => false,
            }
        {
            return Err(());
        }

        self.buf[self.pos] = byte;
        self.pos += 1;

        Ok(())
    }

    pub fn advance(&mut self, byte: u8) -> Utf8DecoderStatus {
        match self.state {
            Utf8DecoderState::New => {
                self.insert_byte(byte).unwrap();

                match self.buf[0].utf8_byte_type() {
                    Utf8ByteType::SingleByte => {
                        self.state = Utf8DecoderState::Done;
                        Utf8DecoderStatus::Done(Utf8Char::new(&self.buf, 1))
                    }
                    Utf8ByteType::StartTwoByte => {
                        self.state = Utf8DecoderState::ExpectingOneByte;
                        Utf8DecoderStatus::Continuation
                    }
                    Utf8ByteType::StartThreeByte => {
                        self.state = Utf8DecoderState::ExpectingTwoBytes;
                        Utf8DecoderStatus::Continuation
                    }
                    Utf8ByteType::StartFourByte => {
                        self.state = Utf8DecoderState::ExpectingThreeBytes;
                        Utf8DecoderStatus::Continuation
                    }
                    Utf8ByteType::Continuation | Utf8ByteType::Invalid => {
                        self.state = Utf8DecoderState::Done;
                        Utf8DecoderStatus::Error
                    }
                }
            }
            Utf8DecoderState::ExpectingOneByte => {
                if self.insert_byte(byte).is_ok() {
                    self.state = Utf8DecoderState::Done;
                    Utf8DecoderStatus::Done(Utf8Char::new(&self.buf, self.pos))
                } else {
                    Utf8DecoderStatus::Error
                }
            }
            Utf8DecoderState::ExpectingTwoBytes => {
                if self.insert_byte(byte).is_ok() {
                    self.state = Utf8DecoderState::ExpectingOneByte;
                    Utf8DecoderStatus::Continuation
                } else {
                    Utf8DecoderStatus::Error
                }
            }
            Utf8DecoderState::ExpectingThreeBytes => {
                if self.insert_byte(byte).is_ok() {
                    self.state = Utf8DecoderState::ExpectingTwoBytes;
                    Utf8DecoderStatus::Continuation
                } else {
                    Utf8DecoderStatus::Error
                }
            }
            Utf8DecoderState::Done => Utf8DecoderStatus::Error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoding_boundaries() {
        for bytes in [
            &b"\xc0\x80"[..],
            b"\xc1\xbf",
            b"\xc2\x80",
            b"\xdf\xbf",
            b"\xe0\x9f\xbf",
            b"\xe0\xa0\x80",
            b"\xed\x9f\xbf",
            b"\xed\xa0\x80",
            b"\xf0\x8f\xbf\xbf",
            b"\xf0\x90\x80\x80",
            b"\xf4\x8f\xbf\xbf",
            b"\xf4\x90\x80\x80",
            b"\xf5\x80\x80\x80",
        ] {
            let mut decoder = Utf8Decoder::new();
            let result = bytes
                .iter()
                .map(|&byte| decoder.advance(byte))
                .find(|status| !matches!(status, Utf8DecoderStatus::Continuation));
            match (core::str::from_utf8(bytes), result) {
                (Ok(_), Some(Utf8DecoderStatus::Done(c))) => assert_eq!(c.as_bytes(), bytes),
                (Err(_), Some(Utf8DecoderStatus::Error)) => {}
                result => panic!("{bytes:?}: {result:?}"),
            }
        }
    }

    #[test]
    fn ascii() {
        let mut parser = Utf8Decoder::new();

        assert_eq!(
            parser.advance(b'a'),
            Utf8DecoderStatus::Done(Utf8Char::from_str("a"))
        );

        assert_eq!(parser.advance(b'a'), Utf8DecoderStatus::Error);
    }

    #[test]
    fn twobyte() {
        let mut parser = Utf8Decoder::new();

        let bytes = "æ".as_bytes();

        assert_eq!(parser.advance(bytes[0]), Utf8DecoderStatus::Continuation);

        assert_eq!(
            parser.advance(bytes[1]),
            Utf8DecoderStatus::Done(Utf8Char::from_str("æ"))
        );

        assert_eq!(parser.advance(b'a'), Utf8DecoderStatus::Error);
    }

    #[test]
    fn threebyte() {
        let mut parser = Utf8Decoder::new();

        let bytes = "€".as_bytes();

        assert_eq!(parser.advance(bytes[0]), Utf8DecoderStatus::Continuation);
        assert_eq!(parser.advance(bytes[1]), Utf8DecoderStatus::Continuation);

        assert_eq!(
            parser.advance(bytes[2]),
            Utf8DecoderStatus::Done(Utf8Char::from_str("€"))
        );

        assert_eq!(parser.advance(b'a'), Utf8DecoderStatus::Error);
    }

    #[test]
    fn fourbyte() {
        let mut parser = Utf8Decoder::new();

        let symbol = "😂";

        let bytes = symbol.as_bytes();
        dbg!(bytes);

        assert_eq!(parser.advance(bytes[0]), Utf8DecoderStatus::Continuation);
        assert_eq!(parser.advance(bytes[1]), Utf8DecoderStatus::Continuation);
        assert_eq!(parser.advance(bytes[2]), Utf8DecoderStatus::Continuation);

        assert_eq!(
            parser.advance(bytes[3]),
            Utf8DecoderStatus::Done(Utf8Char::from_str(symbol))
        );

        assert_eq!(parser.advance(b'a'), Utf8DecoderStatus::Error);
    }

    #[test]
    fn invalid_start() {
        let mut parser = Utf8Decoder::new();

        assert_eq!(parser.advance(0b10000000), Utf8DecoderStatus::Error);
    }

    #[test]
    fn invalid_continuation() {
        let mut parser = Utf8Decoder::new();

        assert_eq!(parser.advance(0b11000010), Utf8DecoderStatus::Continuation);
        assert_eq!(parser.advance(0b00000000), Utf8DecoderStatus::Error);
    }

    #[test]
    fn to_char() {
        assert_eq!(Utf8Char::from_str("€").as_char(), '€');
    }
}
