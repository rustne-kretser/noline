use super::byte::{Utf8Byte, Utf8ByteType};
use super::Utf8Char;

#[derive(Debug, Eq, PartialEq)]
enum Utf8DecoderState {
    New,
    ExpectingOneByte,
    ExpectingTwoBytes,
    ExpectingThreeBytes,
    Done,
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
