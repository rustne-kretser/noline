use super::{OutputItem, UintToBytes};
use crate::terminal::Cursor;

#[cfg_attr(test, derive(Debug))]
enum MoveCursorState {
    New,
    ScrollPrefix,
    Scroll,
    ScrollFinalByte,
    MovePrefix,
    Row,
    Separator,
    Column,
    MoveFinalByte,
    Done,
}

#[cfg_attr(test, derive(Debug))]
pub(super) struct MoveCursor {
    state: MoveCursorState,
    cursor: Cursor,
    scroll: isize,
}

impl MoveCursor {
    pub(super) fn new(cursor: Cursor, scroll: isize) -> Self {
        Self {
            state: MoveCursorState::New,
            cursor,
            scroll,
        }
    }
}

impl Iterator for MoveCursor {
    type Item = OutputItem<'static>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.state {
                MoveCursorState::ScrollPrefix => {
                    self.state = MoveCursorState::Scroll;
                    break Some(OutputItem::Slice("\x1b[".as_bytes()));
                }
                MoveCursorState::Scroll => {
                    self.state = MoveCursorState::ScrollFinalByte;

                    break Some(OutputItem::UintToBytes(
                        UintToBytes::from_uint(self.scroll.unsigned_abs()).unwrap(),
                    ));
                }
                MoveCursorState::ScrollFinalByte => {
                    self.state = MoveCursorState::MovePrefix;

                    break Some(OutputItem::Slice(if self.scroll > 0 {
                        "S".as_bytes()
                    } else {
                        "T".as_bytes()
                    }));
                }
                MoveCursorState::New => {
                    if self.scroll != 0 {
                        self.state = MoveCursorState::ScrollPrefix;
                    } else {
                        self.state = MoveCursorState::MovePrefix;
                    }
                    continue;
                }
                MoveCursorState::MovePrefix => {
                    self.state = MoveCursorState::Row;
                    break Some(OutputItem::Slice("\x1b[".as_bytes()));
                }
                MoveCursorState::Row => {
                    self.state = MoveCursorState::Separator;
                    break Some(OutputItem::UintToBytes(
                        UintToBytes::from_uint(self.cursor.row + 1).unwrap(),
                    ));
                }
                MoveCursorState::Separator => {
                    self.state = MoveCursorState::Column;
                    break Some(OutputItem::Slice(";".as_bytes()));
                }
                MoveCursorState::Column => {
                    self.state = MoveCursorState::MoveFinalByte;

                    break Some(OutputItem::UintToBytes(
                        UintToBytes::from_uint(self.cursor.column + 1).unwrap(),
                    ));
                }
                MoveCursorState::MoveFinalByte => {
                    self.state = MoveCursorState::Done;
                    break Some(OutputItem::Slice("H".as_bytes()));
                }
                MoveCursorState::Done => break None,
            }
        }
    }
}
