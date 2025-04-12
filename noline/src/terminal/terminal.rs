use super::{Cursor, Position};

pub(super) fn distance_from_window(start: isize, end: isize, point: isize) -> isize {
    if point < start {
        point - start
    } else if point > end {
        point - end
    } else {
        0
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
