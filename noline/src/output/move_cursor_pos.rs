use super::move_cursor::MoveCursor;
use crate::terminal::{Position, Terminal};

#[cfg_attr(test, derive(Debug))]
pub(super) enum MoveCursorToPosition {
    Position(Position),
    Move(MoveCursor),
}

impl MoveCursorToPosition {
    pub(super) fn new(position: Position) -> Self {
        Self::Position(position)
    }

    pub(super) fn get_move_cursor(&mut self, terminal: &mut Terminal) -> Option<&mut MoveCursor> {
        loop {
            match self {
                MoveCursorToPosition::Position(position) => {
                    let scroll = terminal.move_cursor(*position);
                    let cursor = terminal.get_cursor();

                    *self = MoveCursorToPosition::Move(MoveCursor::new(cursor, scroll));
                    continue;
                }
                MoveCursorToPosition::Move(move_cursor) => break Some(move_cursor),
            }
        }
    }
}
