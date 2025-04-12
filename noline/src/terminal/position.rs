#[cfg_attr(test, derive(Debug))]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Position {
    pub row: usize,
    pub column: usize,
}

impl Position {
    pub fn new(row: usize, column: usize) -> Self {
        Self { row, column }
    }
}
