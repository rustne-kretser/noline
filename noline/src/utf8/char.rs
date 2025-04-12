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
    pub(super) fn new(bytes: &[u8; 4], len: usize) -> Self {
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
        use super::byte::Utf8ByteType;
        use crate::utf8::byte::Utf8Byte;

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
