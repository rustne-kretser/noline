use super::ToByteVec;

pub(super) struct InputBuilder {
    items: Vec<u8>,
}

impl InputBuilder {
    pub(super) fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub(super) fn add(&mut self, input: impl ToByteVec) {
        self.items.extend(input.to_byte_vec().iter());
    }
}

impl ToByteVec for InputBuilder {
    fn to_byte_vec(self) -> Vec<u8> {
        self.items
    }
}
