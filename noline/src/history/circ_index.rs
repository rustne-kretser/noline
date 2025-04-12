#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
#[cfg_attr(test, derive(Debug))]
pub(super) struct CircularIndex {
    index: usize,
    size: usize,
}

impl CircularIndex {
    pub(super) fn new(index: usize, size: usize) -> Self {
        Self { index, size }
    }

    fn set(&mut self, index: usize) {
        self.index = index;
    }

    fn add(&mut self, value: usize) {
        self.set(self.index + value);
    }

    pub(super) fn increment(&mut self) {
        self.add(1);
    }

    pub(super) fn index(&self) -> usize {
        self.index % self.size
    }

    pub(super) fn diff(&self, other: CircularIndex) -> isize {
        self.index as isize - other.index as isize
    }
}
