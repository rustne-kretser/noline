use super::{ControlCharacter, CSI};
use crate::utf8::Utf8Char;

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
    pub(super) fn escape_sequence(byte: u8) -> Self {
        Action::EscapeSequence(byte)
    }

    pub(super) fn control_character(byte: u8) -> Self {
        Action::ControlCharacter(ControlCharacter::new(byte).unwrap())
    }

    pub(super) fn csi(byte: u8, arg1: Option<usize>, arg2: Option<usize>) -> Self {
        Action::ControlSequenceIntroducer(CSI::new(byte, arg1, arg2))
    }
}
