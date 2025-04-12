use super::circ_range::CircularRange;
use core::{
    iter::{Chain, Zip},
    ops::Range,
    slice,
};

/// Slice of a circular buffer
///
/// Consists of two separate consecutive slices if the circular slice
/// wraps around.
pub struct CircularSlice<'a> {
    buffer: &'a [u8],
    range: CircularRange,
}

impl<'a> CircularSlice<'a> {
    pub(super) fn new(buffer: &'a [u8], start: usize, end: usize, len: usize) -> Self {
        Self::from_range(buffer, CircularRange::new(start, end, len, buffer.len()))
    }

    pub(super) fn from_range(buffer: &'a [u8], range: CircularRange) -> Self {
        Self { buffer, range }
    }

    pub(crate) fn get_ranges(&self) -> (Range<usize>, Range<usize>) {
        self.range.get_ranges()
    }

    pub(crate) fn get_slices(&self) -> (&'a [u8], &'a [u8]) {
        let (range1, range2) = self.get_ranges();

        (&self.buffer[range1], &self.buffer[range2])
    }
}

impl<'a> IntoIterator for CircularSlice<'a> {
    type Item = (usize, &'a u8);

    type IntoIter =
        Chain<Zip<Range<usize>, slice::Iter<'a, u8>>, Zip<Range<usize>, slice::Iter<'a, u8>>>;

    fn into_iter(self) -> Self::IntoIter {
        let (range1, range2) = self.get_ranges();
        let (slice1, slice2) = self.get_slices();

        range1.zip(slice1.iter()).chain(range2.zip(slice2.iter()))
    }
}
