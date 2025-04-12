use core::{iter::Chain, ops::Range};

#[cfg_attr(test, derive(Debug))]
pub(super) enum CircularRange {
    Consecutive(Range<usize>),
    Split(Range<usize>, Range<usize>),
}

impl CircularRange {
    pub(super) fn new(start: usize, end: usize, len: usize, capacity: usize) -> Self {
        assert!(start <= capacity);
        assert!(end <= capacity);

        if len > 0 {
            if start < end {
                Self::Consecutive(start..end)
            } else {
                Self::Split(start..capacity, 0..end)
            }
        } else {
            Self::Consecutive(start..end)
        }
    }

    pub fn get_ranges(&self) -> (Range<usize>, Range<usize>) {
        match self {
            CircularRange::Consecutive(range) => (range.clone(), 0..0),
            CircularRange::Split(range1, range2) => (range1.clone(), range2.clone()),
        }
    }
}

impl IntoIterator for CircularRange {
    type Item = usize;

    type IntoIter = Chain<Range<usize>, Range<usize>>;

    fn into_iter(self) -> Self::IntoIter {
        let (range1, range2) = self.get_ranges();

        range1.chain(range2)
    }
}
