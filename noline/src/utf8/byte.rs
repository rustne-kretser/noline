pub(super) enum Utf8ByteType {
    SingleByte,
    StartTwoByte,
    StartThreeByte,
    StartFourByte,
    Continuation,
    Invalid,
}

pub(super) trait Utf8Byte {
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
        matches!(self.utf8_byte_type(), Utf8ByteType::Continuation)
    }
}
