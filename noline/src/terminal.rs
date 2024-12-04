fn distance_from_window(start: isize, end: isize, point: isize) -> isize {
    if point < start {
        point - start
    } else if point > end {
        point - end
    } else {
        0
    }
}

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

#[cfg_attr(test, derive(Debug, PartialEq, Eq))]
pub struct Terminal {
    rows: usize,
    columns: usize,
    cursor: Cursor,
    row_offset: isize,
}

impl Default for Terminal {
    fn default() -> Self {
        Self::new(24, 80, Cursor::new(0, 0))
    }
}

impl Terminal {
    pub fn new(rows: usize, columns: usize, cursor: Cursor) -> Self {
        let row_offset = -(cursor.row as isize);

        Self {
            rows,
            columns,
            cursor,
            row_offset,
        }
    }

    pub fn resize(&mut self, rows: usize, columns: usize) {
        self.rows = rows;
        self.columns = columns;
    }

    pub fn reset(&mut self, cursor: Cursor) {
        self.cursor = cursor;
        self.row_offset = -(cursor.row as isize);
    }

    pub fn get_cursor(&self) -> Cursor {
        self.cursor
    }

    pub fn get_position(&self) -> Position {
        self.cursor_to_position(self.cursor)
    }

    pub fn scrolling_needed(&self, position: Position) -> isize {
        distance_from_window(
            self.row_offset,
            self.row_offset + self.rows as isize - 1,
            position.row as isize,
        )
    }

    pub fn scroll_to_top(&mut self) -> isize {
        let rows = self.row_offset;
        self.row_offset = 0;

        rows
    }

    pub fn scroll(&mut self, rows: isize) {
        self.row_offset += rows;
    }

    pub fn move_cursor(&mut self, position: Position) -> isize {
        let rows = self.scrolling_needed(position);
        self.scroll(rows);

        #[cfg(test)]
        dbg!(rows, position);

        self.cursor = self
            .position_to_cursor(position)
            .unwrap_or_else(|| unreachable!());

        rows
    }

    pub fn move_cursor_to_start_of_line(&mut self) {
        self.cursor.column = 0;
    }

    pub fn position_to_cursor(&self, position: Position) -> Option<Cursor> {
        let row = position.row as isize - self.row_offset;

        if row >= 0 && row < self.rows as isize {
            Some(Cursor::new(row as usize, position.column))
        } else {
            None
        }
    }

    pub fn cursor_to_position(&self, position: Cursor) -> Position {
        #[cfg(test)]
        dbg!(self.row_offset);

        Position::new(
            (position.row as isize + self.row_offset) as usize,
            position.column,
        )
    }

    pub fn offset_from_position(&self, position: Position) -> isize {
        position.row as isize * self.columns as isize + position.column as isize
    }

    pub fn current_offset(&self) -> isize {
        let position = self.cursor_to_position(self.cursor);
        self.offset_from_position(position)
    }

    fn position_from_offset(&self, offset: isize) -> Position {
        let row = offset.div_euclid(self.columns as isize);
        let column = offset.rem_euclid(self.columns as isize);
        Position::new(row as usize, column as usize)
    }

    pub fn relative_position(&self, steps: isize) -> Position {
        let offset = self.offset_from_position(self.cursor_to_position(self.cursor));

        self.position_from_offset(offset + steps)
    }

    pub fn columns_remaining(&self) -> usize {
        self.columns - self.cursor.column
    }

    #[cfg(test)]
    pub fn get_size(&self) -> (usize, usize) {
        (self.rows, self.columns)
    }
}

#[cfg(test)]
mod tests {
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
}
