use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::utf8::{Utf8Char, Utf8Decoder, Utf8DecoderStatus};

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Eq, PartialEq, Copy, Clone, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum ControlCharacter {
    NUL = 0x0,
    CtrlA = 0x1,
    CtrlB = 0x2,
    CtrlC = 0x3,
    CtrlD = 0x4,
    CtrlE = 0x5,
    CtrlF = 0x6,
    CtrlG = 0x7,
    CtrlH = 0x8,
    Tab = 0x9,
    LineFeed = 0xA,
    CtrlK = 0xB,
    CtrlL = 0xC,
    CarriageReturn = 0xD,
    CtrlN = 0xE,
    CtrlO = 0xF,
    CtrlP = 0x10,
    CtrlQ = 0x11,
    CtrlR = 0x12,
    CtrlS = 0x13,
    CtrlT = 0x14,
    CtrlU = 0x15,
    CtrlV = 0x16,
    CtrlW = 0x17,
    CtrlX = 0x18,
    CtrlY = 0x19,
    CtrlZ = 0x1A,
    Escape = 0x1B,
    FS = 0x1C,
    GS = 0x1D,
    RS = 0x1E,
    US = 0x1F,
    Backspace = 0x7F,
}

impl ControlCharacter {
    fn new(byte: u8) -> Result<Self, ()> {
        match Self::try_from(byte) {
            Ok(this) => Ok(this),
            Err(_) => Err(()),
        }
    }
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum CSI {
    CUU(usize),
    CUD(usize),
    CUF(usize),
    CUB(usize),
    CPR(usize, usize),
    CUP(usize, usize),
    ED(usize),
    DSR,
    SU(usize),
    SD(usize),
    Home,
    Delete,
    End,
    Unknown(u8),
}

impl CSI {
    fn new(byte: u8, arg1: Option<usize>, arg2: Option<usize>) -> Option<Self> {
        let c = byte as char;

        Some(match c {
            'A' => Self::CUU(arg1.unwrap_or(1)),
            'B' => Self::CUD(arg1.unwrap_or(1)),
            'C' => Self::CUF(arg1.unwrap_or(1)),
            'D' => Self::CUB(arg1.unwrap_or(1)),
            'H' => Self::CUP(arg1.unwrap_or(1), arg2.unwrap_or(1)),
            'J' => Self::ED(arg1.unwrap_or(0)),
            'R' => Self::CPR(arg1.unwrap(), arg2.unwrap()),
            'S' => Self::SU(arg1.unwrap_or(1)),
            'T' => Self::SD(arg1.unwrap_or(1)),
            'n' => Self::DSR,
            '~' => {
                if let Some(arg) = arg1 {
                    match arg {
                        1 => Self::Home,
                        3 => Self::Delete,
                        4 => Self::End,
                        _ => Self::Unknown(byte),
                    }
                } else {
                    Self::Unknown(byte)
                }
            }
            _ => Self::Unknown(byte),
        })
    }
}

#[cfg_attr(test, derive(Debug))]
#[derive(Eq, PartialEq, Copy, Clone)]
pub enum Action {
    Ignore,
    Print(Utf8Char),
    InvalidUtf8,
    ControlCharacter(ControlCharacter),
    EscapeSequence(u8),
    ControlSequenceIntroducer(CSI),
}

impl Action {
    fn escape_sequence(byte: u8) -> Self {
        Action::EscapeSequence(byte)
    }

    fn control_character(byte: u8) -> Self {
        Action::ControlCharacter(ControlCharacter::new(byte).unwrap())
    }

    fn csi(byte: u8, arg1: Option<usize>, arg2: Option<usize>) -> Self {
        Action::ControlSequenceIntroducer(CSI::new(byte, arg1, arg2).unwrap())
    }
}

#[derive(Debug, Eq, PartialEq)]
enum State {
    Ground,
    Utf8Sequence(Option<Utf8Decoder>),
    EscapeSequence,
    CSIStart,
    CSIArg1(Option<usize>),
    CSIArg2(Option<usize>, Option<usize>),
}

pub struct Parser {
    state: State,
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

#[cfg(test)]
pub(crate) mod tests {
    use crate::testlib::ToByteVec;

    use super::*;
    use std::vec::Vec;
    use ControlCharacter::*;

    fn input_sequence(parser: &mut Parser, seq: impl ToByteVec) -> Vec<Action> {
        seq.to_byte_vec()
            .into_iter()
            .map(|b| parser.advance(b))
            .collect()
    }

    #[test]
    fn parser() {
        let mut parser = Parser::new();

        assert_eq!(parser.state, State::Ground);

        assert_eq!(parser.advance(b'a'), Action::Print(Utf8Char::from_str("a")));
        assert_eq!(parser.advance(0x7), Action::ControlCharacter(CtrlG));
        assert_eq!(parser.advance(0x3), Action::ControlCharacter(CtrlC));

        let actions = input_sequence(&mut parser, "æ");
        assert_eq!(
            actions,
            [Action::Ignore, Action::Print(Utf8Char::from_str("æ"))]
        );

        let mut actions = input_sequence(&mut parser, "\x1b[312;836R");
        assert_eq!(
            actions.pop().unwrap(),
            Action::ControlSequenceIntroducer(CSI::CPR(312, 836))
        );
        while let Some(action) = actions.pop() {
            assert_eq!(action, Action::Ignore);
        }

        let mut actions = input_sequence(&mut parser, "\x1b[A");

        assert_eq!(
            actions.pop().unwrap(),
            Action::ControlSequenceIntroducer(CSI::CUU(1))
        );

        let mut actions = input_sequence(&mut parser, "\x1b[10B");

        assert_eq!(
            actions.pop().unwrap(),
            Action::ControlSequenceIntroducer(CSI::CUD(10))
        );

        let mut actions = input_sequence(&mut parser, "\x1b[H");

        assert_eq!(
            actions.pop().unwrap(),
            Action::ControlSequenceIntroducer(CSI::CUP(1, 1))
        );

        let mut actions = input_sequence(&mut parser, "\x1b[2;5H");

        assert_eq!(
            actions.pop().unwrap(),
            Action::ControlSequenceIntroducer(CSI::CUP(2, 5))
        );

        let mut actions = input_sequence(&mut parser, "\x1b[;5H");

        assert_eq!(
            actions.pop().unwrap(),
            Action::ControlSequenceIntroducer(CSI::CUP(1, 5))
        );

        let mut actions = input_sequence(&mut parser, "\x1b[17;H");

        assert_eq!(
            actions.pop().unwrap(),
            Action::ControlSequenceIntroducer(CSI::CUP(17, 1))
        );

        let mut actions = input_sequence(&mut parser, "\x1b[;H");

        assert_eq!(
            actions.pop().unwrap(),
            Action::ControlSequenceIntroducer(CSI::CUP(1, 1))
        );

        let mut actions = input_sequence(&mut parser, "\x1b[;10H");

        assert_eq!(
            actions.pop().unwrap(),
            Action::ControlSequenceIntroducer(CSI::CUP(1, 10))
        );
    }
}
