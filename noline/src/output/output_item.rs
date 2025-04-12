use super::UintToBytes;

#[cfg_attr(test, derive(Debug))]
pub enum OutputItem<'a> {
    Slice(&'a [u8]),
    UintToBytes(UintToBytes<4>),
    EndOfString,
    Abort,
}

impl<'a> OutputItem<'a> {
    pub fn get_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Slice(slice) => Some(slice),
            Self::UintToBytes(uint) => Some(uint.as_bytes()),
            Self::EndOfString | Self::Abort => None,
        }
    }
}
