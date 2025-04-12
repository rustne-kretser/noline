use super::{circ_range::CircularRange, history_nav::HistoryNavigator};
use std::string::String;
use std::vec::Vec;

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
fn navigator() {
    let mut history = UnboundedHistory::new();
    let mut navigator = HistoryNavigator::new(&mut history);

    assert!(navigator.move_up().is_err());
    assert!(navigator.move_down().is_err());

    navigator.history.add_entry("line 1").unwrap();
    navigator.reset();

    assert!(navigator.move_up().is_ok());
    assert!(navigator.move_up().is_err());

    assert!(navigator.move_down().is_err());
}
