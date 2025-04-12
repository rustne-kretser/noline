use crate::input::ControlCharacter;

pub trait ToByteVec {
    fn to_byte_vec(self) -> Vec<u8>;
}

impl ToByteVec for &str {
    fn to_byte_vec(self) -> Vec<u8> {
        self.bytes().collect()
    }
}

impl ToByteVec for ControlCharacter {
    fn to_byte_vec(self) -> Vec<u8> {
        [self.into()].into_iter().collect()
    }
}

impl ToByteVec for Vec<ControlCharacter> {
    fn to_byte_vec(self) -> Vec<u8> {
        self.into_iter().map(|c| c.into()).collect()
    }
}

impl<const N: usize> ToByteVec for [ControlCharacter; N] {
    fn to_byte_vec(self) -> Vec<u8> {
        self.into_iter().map(|c| c.into()).collect()
    }
}

impl ToByteVec for Vec<&str> {
    fn to_byte_vec(self) -> Vec<u8> {
        self.into_iter()
            .flat_map(|s| s.as_bytes().iter().copied())
            .collect()
    }
}

impl<const N: usize> ToByteVec for [&str; N] {
    fn to_byte_vec(self) -> Vec<u8> {
        self.into_iter()
            .flat_map(|s| s.as_bytes().iter().copied())
            .collect()
    }
}
