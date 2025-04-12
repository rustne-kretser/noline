use super::*;

#[test]
fn test_distance_from_window() {
    assert_eq!(distance_from_window(4, 8, 2), -2);
    assert_eq!(distance_from_window(4, 8, 4), 0);
    assert_eq!(distance_from_window(4, 8, 8), 0);
    assert_eq!(distance_from_window(4, 8, 10), 2);

    assert_eq!(distance_from_window(-3, 8, 2), 0);
    assert_eq!(distance_from_window(-3, 8, -5), -2);
    assert_eq!(distance_from_window(-3, 8, 9), 1);
}

#[test]
fn position_from_top() {
    let term = Terminal::new(4, 10, Cursor::new(0, 0));

    assert_eq!(
        term.cursor_to_position(term.get_cursor()),
        Position::new(0, 0)
    );

    assert_eq!(
        term.cursor_to_position(Cursor::new(3, 9)),
        Position::new(3, 9)
    );

    assert_eq!(
        term.cursor_to_position(Cursor::new(4, 9)),
        Position::new(4, 9)
    );

    assert_eq!(
        term.position_to_cursor(Position::new(3, 9)),
        Some(Cursor::new(3, 9))
    );

    assert_eq!(term.position_to_cursor(Position::new(4, 9)), None);
}

#[test]
fn position_from_second_line() {
    let term = Terminal::new(4, 10, Cursor::new(1, 0));

    assert_eq!(
        term.cursor_to_position(term.get_cursor()),
        Position::new(0, 0)
    );

    assert_eq!(
        term.cursor_to_position(Cursor::new(3, 9)),
        Position::new(2, 9)
    );

    assert_eq!(
        term.position_to_cursor(Position::new(2, 9)),
        Some(Cursor::new(3, 9))
    );
}

#[test]
fn position_scroll() {
    let mut term = Terminal::new(4, 10, Cursor::new(0, 0));

    assert_eq!(term.move_cursor(Position::new(7, 0)), 4);

    assert_eq!(
        term.cursor_to_position(term.get_cursor()),
        Position::new(7, 0)
    );

    assert_eq!(
        term.cursor_to_position(Cursor::new(3, 9)),
        Position::new(7, 9)
    );

    assert_eq!(
        term.cursor_to_position(Cursor::new(0, 0)),
        Position::new(4, 0)
    );

    assert_eq!(term.position_to_cursor(Position::new(2, 9)), None);
}

#[test]
fn position_scroll_offset() {
    let mut term = Terminal::new(4, 10, Cursor::new(3, 9));

    let position = term.relative_position(1);

    assert_eq!(position, Position::new(1, 0));
    assert_eq!(term.position_to_cursor(position), None);

    assert_eq!(term.move_cursor(Position::new(1, 0)), 1);
}

#[test]
fn move_cursor() {
    let mut term = Terminal::new(4, 10, Cursor::new(0, 0));

    let pos = Position::new(3, 9);
    assert_eq!(term.scrolling_needed(pos), 0);

    assert_eq!(term.move_cursor(pos), 0);

    let pos = Position::new(4, 0);
    assert_eq!(term.scrolling_needed(pos), 1);

    assert_eq!(term.move_cursor(pos), 1);

    assert_eq!(term.get_cursor(), Cursor::new(3, 0));
    assert_eq!(term.get_position(), Position::new(4, 0));
    assert_eq!(term.current_offset(), 40);

    let pos = Position::new(0, 0);
    assert_eq!(term.scrolling_needed(pos), -1);

    assert_eq!(term.move_cursor(pos), -1);

    assert_eq!(term.get_cursor(), Cursor::new(0, 0));
    assert_eq!(term.get_position(), Position::new(0, 0));
}

#[test]
fn offset() {
    let term = Terminal::new(4, 10, Cursor::new(1, 0));

    assert_eq!(term.get_cursor(), Cursor::new(1, 0));
    assert_eq!(term.get_position(), Position::new(0, 0));
    assert_eq!(term.current_offset(), 0);
}
