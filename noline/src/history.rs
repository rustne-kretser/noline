//! Line history

use core::{
    iter::{Chain, Zip},
    ops::Range,
    slice,
};

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
#[cfg_attr(test, derive(Debug))]
struct CircularIndex {
    index: usize,
    size: usize,
}

impl CircularIndex {
    fn new(index: usize, size: usize) -> Self {
        Self { index, size }
    }

    fn set(&mut self, index: usize) {
        self.index = index;
    }

    fn add(&mut self, value: usize) {
        self.set(self.index + value);
    }

    fn increment(&mut self) {
        self.add(1);
    }

    fn index(&self) -> usize {
        self.index % self.size
    }

    fn diff(&self, other: CircularIndex) -> isize {
        self.index as isize - other.index as isize
    }
}

struct Window {
    size: usize,
    start: CircularIndex,
    end: CircularIndex,
}

impl Window {
    fn new(size: usize) -> Self {
        let start = CircularIndex::new(0, size);
        let end = CircularIndex::new(0, size);
        Self { size, start, end }
    }

    fn len(&self) -> usize {
        self.end.diff(self.start) as usize
    }

    fn widen(&mut self) {
        self.end.increment();

        if self.end.diff(self.start) as usize > self.size {
            self.start.increment();
        }
    }

    fn narrow(&mut self) {
        if self.end.diff(self.start) > 0 {
            self.start.increment();
        }
    }

    fn start(&self) -> usize {
        self.start.index()
    }

    fn end(&self) -> usize {
        self.end.index()
    }
}

#[cfg_attr(test, derive(Debug))]
enum CircularRange {
    Consecutive(Range<usize>),
    Split(Range<usize>, Range<usize>),
}

impl CircularRange {
    fn new(start: usize, end: usize, len: usize, capacity: usize) -> Self {
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

/// Slice of a circular buffer
///
/// Consists of two separate consecutive slices if the circular slice
/// wraps around.
pub struct CircularSlice<'a> {
    buffer: &'a [u8],
    range: CircularRange,
}

impl<'a> CircularSlice<'a> {
    fn new(buffer: &'a [u8], start: usize, end: usize, len: usize) -> Self {
        Self::from_range(buffer, CircularRange::new(start, end, len, buffer.len()))
    }

    fn from_range(buffer: &'a [u8], range: CircularRange) -> Self {
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

/// Trait for line history
pub trait History {
    /// Return entry at index, or None if out of bounds
    fn get_entry(&self, index: usize) -> Option<CircularSlice<'_>>;

    /// Add new entry at the end
    fn add_entry<'a>(&mut self, entry: &'a str) -> Result<(), &'a str>;

    /// Return number of entries in history
    fn number_of_entries(&self) -> usize;

    /// Add entries from an iterator
    fn load_entries<'a, I: Iterator<Item = &'a str>>(&mut self, entries: I) -> usize {
        entries
            .take_while(|entry| self.add_entry(entry).is_ok())
            .count()
    }
}

/// Return an iterator over history entries
///
/// # Note
///
/// This should ideally be in the [`History`] trait, but is
/// until `type_alias_impl_trait` is stable.
pub(crate) fn get_history_entries<H: History>(
    history: &H,
) -> impl Iterator<Item = CircularSlice<'_>> {
    (0..(history.number_of_entries())).filter_map(|index| history.get_entry(index))
}

/// Static history backed by array
pub struct SliceHistory<'a> {
    buffer: &'a mut [u8],
    window: Window,
}

impl<'a> SliceHistory<'a> {
    /// Create new static history
    pub fn new(buffer: &'a mut [u8]) -> Self {
        Self {
            window: Window::new(buffer.len()),
            buffer,
        }
    }

    fn get_available_range(&self) -> CircularRange {
        let len = self.buffer.len();
        CircularRange::new(self.window.end(), self.window.end(), len, len)
    }

    fn get_buffer(&self) -> CircularSlice<'_> {
        CircularSlice::new(
            self.buffer,
            self.window.start(),
            self.window.end(),
            self.window.len(),
        )
    }

    fn get_entry_ranges(&self) -> impl Iterator<Item = CircularRange> + '_ {
        let delimeters =
            self.get_buffer()
                .into_iter()
                .filter_map(|(index, b)| if *b == 0x0 { Some(index) } else { None });

        [self.window.start()]
            .into_iter()
            .chain(delimeters.clone().map(|i| i + 1))
            .zip(delimeters.chain([self.window.end()]))
            .filter_map(|(start, end)| {
                if start != end {
                    Some(CircularRange::new(
                        start,
                        end,
                        self.window.len(),
                        self.buffer.len(),
                    ))
                } else {
                    None
                }
            })
    }

    fn get_entries(&self) -> impl Iterator<Item = CircularSlice<'_>> {
        self.get_entry_ranges()
            .map(|range| CircularSlice::from_range(self.buffer, range))
    }
}

impl<'a> History for SliceHistory<'a> {
    fn add_entry<'b>(&mut self, entry: &'b str) -> Result<(), &'b str> {
        if entry.len() + 1 > self.buffer.len() {
            return Err(entry);
        }

        for (_, b) in self
            .get_available_range()
            .into_iter()
            .zip(entry.as_bytes().iter())
        {
            self.buffer[self.window.end()] = *b;
            self.window.widen();
        }

        if self.buffer[self.window.end()] != 0x0 {
            self.buffer[self.window.end()] = 0x0;

            self.window.widen();

            while self.buffer[self.window.start()] != 0x0 {
                self.window.narrow();
            }
        } else {
            self.window.widen();
        }

        Ok(())
    }

    fn number_of_entries(&self) -> usize {
        self.get_entries().count()
    }

    fn get_entry(&self, index: usize) -> Option<CircularSlice<'_>> {
        self.get_entries().nth(index)
    }
}

/// Emtpy implementation for Editors with no history
pub struct NoHistory {}

impl NoHistory {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for NoHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl History for NoHistory {
    fn get_entry(&self, _index: usize) -> Option<CircularSlice<'_>> {
        None
    }

    fn add_entry<'a>(&mut self, entry: &'a str) -> Result<(), &'a str> {
        Err(entry)
    }

    fn number_of_entries(&self) -> usize {
        0
    }
}

/// Wrapper used for history navigation in [`core::Line`]
pub(crate) struct HistoryNavigator<'a, H: History> {
    pub(crate) history: &'a mut H,
    position: Option<usize>,
}

impl<'a, H: History> HistoryNavigator<'a, H> {
    pub(crate) fn new(history: &'a mut H) -> Self {
        Self {
            history,
            position: None,
        }
    }

    fn set_position(&mut self, position: usize) -> usize {
        *self.position.insert(position)
    }

    fn get_position(&mut self) -> usize {
        *self
            .position
            .get_or_insert_with(|| self.history.number_of_entries())
    }

    pub(crate) fn move_up(&mut self) -> Result<CircularSlice<'_>, ()> {
        let position = self.get_position();

        if position > 0 {
            let position = self.set_position(position - 1);

            Ok(self.history.get_entry(position).unwrap())
        } else {
            Err(())
        }
    }

    pub(crate) fn move_down(&mut self) -> Result<CircularSlice<'_>, ()> {
        let new_position = self.get_position() + 1;

        if new_position < self.history.number_of_entries() {
            let position = self.set_position(new_position);

            Ok(self.history.get_entry(position).unwrap())
        } else {
            Err(())
        }
    }

    pub(crate) fn reset(&mut self) {
        self.position = None;
    }

    pub(crate) fn is_active(&self) -> bool {
        self.position.is_some()
    }
}

#[cfg(any(test, doc, feature = "alloc", feature = "std"))]
mod alloc {

    use super::*;
    use alloc::{
        string::{String, ToString},
        collections::VecDeque,
    };

    extern crate alloc;

    pub struct AllocHistory {
        pub(crate) buffer: VecDeque<String>,
        max_entries: usize,
    }

    impl AllocHistory {
        pub fn new(max_size: usize) -> Self {
            Self {
                buffer: VecDeque::new(),
                max_entries: max_size,
            }
        }
    }

    impl History for AllocHistory {
        fn get_entry(&self, index: usize) -> Option<CircularSlice<'_>> {
            let s = self.buffer[index].as_str();

            Some(CircularSlice::new(s.as_bytes(), 0, s.len(), s.len()))
        }

        fn add_entry<'a>(&mut self, entry: &'a str) -> Result<(), &'a str> {
            self.buffer.push_back(entry.to_string());

            if self.buffer.len() > self.max_entries {
                self.buffer.pop_front();
            }

            Ok(())
        }

        fn number_of_entries(&self) -> usize {
            self.buffer.len()
        }
    }
}

#[cfg(any(test, doc, feature = "alloc", feature = "std"))]
pub use alloc::AllocHistory;

#[cfg(test)]
mod tests {
    use std::vec::Vec;

    use std::string::String;

    use super::*;

    impl<'a> FromIterator<CircularSlice<'a>> for Vec<String> {
        fn from_iter<T: IntoIterator<Item = CircularSlice<'a>>>(iter: T) -> Self {
            iter.into_iter()
                .map(|circular| {
                    let bytes = circular.into_iter().map(|(_, b)| *b).collect::<Vec<u8>>();
                    String::from_utf8(bytes).unwrap()
                })
                .collect()
        }
    }

    #[test]
    fn circular_range() {
        assert_eq!(CircularRange::new(0, 3, 10, 10).get_ranges(), (0..3, 0..0));
        assert_eq!(CircularRange::new(0, 0, 10, 10).get_ranges(), (0..10, 0..0));
        assert_eq!(CircularRange::new(0, 0, 0, 10).get_ranges(), (0..0, 0..0));
        assert_eq!(CircularRange::new(7, 3, 10, 10).get_ranges(), (7..10, 0..3));
        assert_eq!(CircularRange::new(0, 0, 10, 10).get_ranges(), (0..10, 0..0));
        assert_eq!(
            CircularRange::new(0, 10, 10, 10).get_ranges(),
            (0..10, 0..0)
        );
        assert_eq!(CircularRange::new(9, 9, 10, 10).get_ranges(), (9..10, 0..9));
        assert_eq!(
            CircularRange::new(10, 10, 10, 10).get_ranges(),
            (10..10, 0..10)
        );

        assert_eq!(CircularRange::new(0, 10, 10, 10).into_iter().count(), 10);
        assert_eq!(CircularRange::new(10, 10, 10, 10).into_iter().count(), 10);
        assert_eq!(CircularRange::new(4, 4, 10, 10).into_iter().count(), 10);
    }

    #[test]
    fn circular_slice() {
        assert_eq!(
            CircularSlice::new("abcdef".as_bytes(), 0, 3, 6).get_slices(),
            ("abc".as_bytes(), "".as_bytes())
        );

        assert_eq!(
            CircularSlice::new("abcdef".as_bytes(), 3, 0, 6).get_slices(),
            ("def".as_bytes(), "".as_bytes())
        );

        assert_eq!(
            CircularSlice::new("abcdef".as_bytes(), 3, 3, 6).get_slices(),
            ("def".as_bytes(), "abc".as_bytes())
        );

        assert_eq!(
            CircularSlice::new("abcdef".as_bytes(), 0, 6, 6).get_slices(),
            ("abcdef".as_bytes(), "".as_bytes())
        );

        assert_eq!(
            CircularSlice::new("abcdef".as_bytes(), 0, 0, 6).get_slices(),
            ("abcdef".as_bytes(), "".as_bytes())
        );

        assert_eq!(
            CircularSlice::new("abcdef".as_bytes(), 0, 0, 0).get_slices(),
            ("".as_bytes(), "".as_bytes())
        );

        assert_eq!(
            CircularSlice::new("abcdef".as_bytes(), 6, 6, 6).get_slices(),
            ("".as_bytes(), "abcdef".as_bytes())
        );
    }

    #[test]
    fn static_history() {
        let mut buffer = [0; 10];
        let mut history: SliceHistory = SliceHistory::new(&mut buffer);

        assert_eq!(history.get_available_range().get_ranges(), (0..10, 0..0));

        assert_eq!(
            history.get_entries().collect::<Vec<String>>(),
            Vec::<String>::new()
        );

        history.add_entry("abc").unwrap();

        // dbg!(history.start, history.end, history.len);
        // dbg!(history.get_entry_ranges().collect::<Vec<_>>());
        // dbg!(history.buffer);

        assert_eq!(history.get_entries().collect::<Vec<String>>(), vec!["abc"]);

        history.add_entry("def").unwrap();

        // dbg!(history.buffer);

        assert_eq!(
            history.get_entries().collect::<Vec<String>>(),
            vec!["abc", "def"]
        );

        history.add_entry("ghi").unwrap();

        dbg!(
            history.window.start(),
            history.window.end(),
            history.window.len()
        );

        assert_eq!(
            history.get_entries().collect::<Vec<String>>(),
            vec!["def", "ghi"]
        );

        history.add_entry("j").unwrap();

        // dbg!(history.start, history.end, history.len);

        assert_eq!(
            history.get_entries().collect::<Vec<String>>(),
            vec!["def", "ghi", "j"]
        );

        history.add_entry("012345678").unwrap();

        assert_eq!(
            history.get_entries().collect::<Vec<String>>(),
            vec!["012345678"]
        );

        assert!(history.add_entry("0123456789").is_err());

        history.add_entry("abc").unwrap();

        assert_eq!(history.get_entries().collect::<Vec<String>>(), vec!["abc"]);

        history.add_entry("defgh").unwrap();

        assert_eq!(
            history.get_entries().collect::<Vec<String>>(),
            vec!["abc", "defgh"]
        );
    }

    #[test]
    fn alloc_history() {
        let mut history = AllocHistory::new(3);

        history.add_entry("0").unwrap();
        history.add_entry("1").unwrap();
        history.add_entry("2").unwrap();

        assert_eq!(history.buffer,
                   vec!["0", "1", "2"]);

        history.add_entry("3").unwrap();

        assert_eq!(history.buffer,
                   vec!["1", "2", "3"]);
    }

    #[test]
    fn navigator() {
        let mut history = AllocHistory::new(10);
        let mut navigator = HistoryNavigator::new(&mut history);

        assert!(navigator.move_up().is_err());
        assert!(navigator.move_down().is_err());

        navigator.history.add_entry("line 1").unwrap();
        navigator.reset();

        assert!(navigator.move_up().is_ok());
        assert!(navigator.move_up().is_err());

        assert!(navigator.move_down().is_err());
    }
}
