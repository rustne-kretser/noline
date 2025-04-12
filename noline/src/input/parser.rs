use super::{state::State, Action};
use crate::utf8::{Utf8Decoder, Utf8DecoderStatus};

pub struct Parser {
    pub(super) state: State,
}

impl Parser {
    pub fn new() -> Self {
        Self {
            state: State::Ground,
        }
    }

    pub fn advance(&mut self, byte: u8) -> Action {
        match self.state {
            State::Ground => match byte {
                0x1b => {
                    self.state = State::EscapeSequence;
                    Action::Ignore
                }
                0x0..=0x1a | 0x1c..=0x1f | 0x7f => Action::control_character(byte),
                0x20..=0x7e | 0x80..=0xff => {
                    let mut decoder = Utf8Decoder::new();

                    match decoder.advance(byte) {
                        Utf8DecoderStatus::Continuation => {
                            self.state = State::Utf8Sequence(Some(decoder));
                            Action::Ignore
                        }
                        Utf8DecoderStatus::Done(c) => Action::Print(c),
                        Utf8DecoderStatus::Error => Action::InvalidUtf8,
                    }
                }
            },
            State::Utf8Sequence(ref mut decoder) => {
                let mut decoder = decoder.take().unwrap();

                match decoder.advance(byte) {
                    Utf8DecoderStatus::Continuation => {
                        self.state = State::Utf8Sequence(Some(decoder));
                        Action::Ignore
                    }
                    Utf8DecoderStatus::Done(c) => {
                        self.state = State::Ground;
                        Action::Print(c)
                    }
                    Utf8DecoderStatus::Error => {
                        self.state = State::Ground;
                        Action::InvalidUtf8
                    }
                }
            }
            State::EscapeSequence => {
                if byte == 0x5b {
                    self.state = State::CSIStart;
                    Action::Ignore
                } else {
                    self.state = State::Ground;
                    Action::escape_sequence(byte)
                }
            }
            State::CSIStart => match byte {
                0x30..=0x39 => {
                    let value: usize = (byte - 0x30) as usize;
                    self.state = State::CSIArg1(Some(value));
                    Action::Ignore
                }
                0x3b => {
                    self.state = State::CSIArg2(None, None);
                    Action::Ignore
                }
                0x40..=0x7e => {
                    self.state = State::Ground;
                    Action::csi(byte, None, None)
                }
                _ => Action::Ignore,
            },
            State::CSIArg1(value) => match byte {
                0x30..=0x39 => {
                    let value: usize = value.unwrap_or(0) * 10 + (byte - 0x30) as usize;
                    self.state = State::CSIArg1(Some(value));
                    Action::Ignore
                }
                0x3b => {
                    self.state = State::CSIArg2(value, None);
                    Action::Ignore
                }
                0x40..=0x7e => {
                    self.state = State::Ground;
                    Action::csi(byte, value, None)
                }
                _ => Action::Ignore,
            },
            State::CSIArg2(arg1, arg2) => match byte {
                0x30..=0x39 => {
                    let arg2: usize = arg2.unwrap_or(0) * 10 + (byte - 0x30) as usize;
                    self.state = State::CSIArg2(arg1, Some(arg2));
                    Action::Ignore
                }
                0x40..=0x7e => {
                    self.state = State::Ground;
                    Action::csi(byte, arg1, arg2)
                }
                _ => Action::Ignore,
            },
        }
    }
}
