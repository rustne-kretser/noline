#[cfg_attr(test, derive(Debug))]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Cursor {
    pub row: usize,
    pub column: usize,
}

impl Cursor {
    pub fn new(row: usize, column: usize) -> Self {
        Self { row, column }
    }
}
