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
        } else if byte & 0b11100000 == 0b11000000 {
            Utf8ByteType::StartTwoByte
        } else if byte & 0b11110000 == 0b11100000 {
            Utf8ByteType::StartThreeByte
        } else if byte & 0b11111000 == 0b11110000 {
            Utf8ByteType::StartFourByte
        } else {
            Utf8ByteType::Invalid
        }
    }

    fn utf8_is_continuation(&self) -> bool {
        if let Utf8ByteType::Continuation = self.utf8_byte_type() {
            true
        } else {
            false
        }
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
        f.debug_tuple("Utf8Char").field(&self.to_char()).finish()
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
    pub(crate) fn to_char(&self) -> char {
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

        self.buf[self.pos] = byte;
        self.pos += 1;

        return Ok(());
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
    fn ascii() {
        let mut parser = Utf8Decoder::new();

        assert_eq!(
            parser.advance('a' as u8),
            Utf8DecoderStatus::Done(Utf8Char::from_str("a"))
        );

        assert_eq!(parser.advance('a' as u8), Utf8DecoderStatus::Error);
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

        assert_eq!(parser.advance('a' as u8), Utf8DecoderStatus::Error);
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

        assert_eq!(parser.advance('a' as u8), Utf8DecoderStatus::Error);
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

        assert_eq!(parser.advance('a' as u8), Utf8DecoderStatus::Error);
    }

    #[test]
    fn invalid_start() {
        let mut parser = Utf8Decoder::new();

        assert_eq!(parser.advance(0b10000000), Utf8DecoderStatus::Error);
    }

    #[test]
    fn invalid_continuation() {
        let mut parser = Utf8Decoder::new();

        assert_eq!(parser.advance(0b11000000), Utf8DecoderStatus::Continuation);
        assert_eq!(parser.advance(0b00000000), Utf8DecoderStatus::Error);
    }

    #[test]
    fn to_char() {
        assert_eq!(Utf8Char::from_str("€").to_char(), '€');
    }
}
