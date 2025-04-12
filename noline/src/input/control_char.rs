use num_enum::{IntoPrimitive, TryFromPrimitive};

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
    pub(super) fn new(byte: u8) -> Result<Self, ()> {
        match Self::try_from(byte) {
            Ok(this) => Ok(this),
            Err(_) => Err(()),
        }
    }
}
