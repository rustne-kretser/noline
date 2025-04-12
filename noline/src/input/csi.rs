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
    Invalid,
}

impl CSI {
    pub(super) fn new(byte: u8, arg1: Option<usize>, arg2: Option<usize>) -> Self {
        let c = byte as char;

        match c {
            'A' => Self::CUU(arg1.unwrap_or(1)),
            'B' => Self::CUD(arg1.unwrap_or(1)),
            'C' => Self::CUF(arg1.unwrap_or(1)),
            'D' => Self::CUB(arg1.unwrap_or(1)),
            'H' => Self::CUP(arg1.unwrap_or(1), arg2.unwrap_or(1)),
            'J' => Self::ED(arg1.unwrap_or(0)),
            'R' => {
                if let (Some(arg1), Some(arg2)) = (arg1, arg2) {
                    Self::CPR(arg1, arg2)
                } else {
                    Self::Invalid
                }
            }
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
        }
    }
}
