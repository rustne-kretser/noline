use super::{
    move_cursor_pos::MoveCursorToPosition,
    printable::{Printable, PrintableItem},
    step::Step::*,
    OutputItem,
};
use crate::terminal::Terminal;

// #[cfg_attr(test, derive(Debug))]
pub(super) enum Step<'a, I> {
    Print(Printable<'a, I>),
    Move(MoveCursorToPosition),
    MoveCursorToEdge,
    GetPosition,
    SavePosition,
    RestorePosition,
    ClearLine,
    Erase,
    Newline,
    Bell,
    EndOfString,
    Abort,
    Done,
}

impl<'a, 'item, I> Step<'a, I>
where
    I: Iterator<Item = &'item str>,
    'item: 'a,
{
    fn transition(
        &mut self,
        new_state: Step<'a, I>,
        output: OutputItem<'a>,
    ) -> Option<OutputItem<'a>> {
        *self = new_state;
        Some(output)
    }

    pub(super) fn advance(&mut self, terminal: &mut Terminal) -> Option<OutputItem<'a>> {
        match self {
            Print(printable) => {
                if let Some(item) = printable.next_item(terminal.columns_remaining()) {
                    let s = match item {
                        PrintableItem::Str(s) => {
                            let position = terminal.relative_position(s.chars().count() as isize);
                            terminal.move_cursor(position);

                            s
                        }
                        PrintableItem::Newline => "\n\r",
                    };

                    Some(OutputItem::Slice(s.as_bytes()))
                } else {
                    *self = Step::Done;
                    None
                }
            }
            Move(pos) => {
                if let Some(move_cursor) = pos.get_move_cursor(terminal) {
                    if let Some(byte) = move_cursor.next() {
                        return Some(byte);
                    }
                }

                *self = Step::Done;
                None
            }
            MoveCursorToEdge => self.transition(Step::Done, OutputItem::Slice(b"\x1b[999;999H")),
            Erase => self.transition(Step::Done, OutputItem::Slice("\x1b[J".as_bytes())),
            Newline => {
                let mut position = terminal.get_position();
                position.row += 1;
                position.column = 0;
                terminal.move_cursor(position);

                self.transition(Step::Done, OutputItem::Slice("\n\r".as_bytes()))
            }
            Bell => self.transition(Step::Done, OutputItem::Slice("\x07".as_bytes())),
            EndOfString => self.transition(Step::Done, OutputItem::EndOfString),
            Abort => self.transition(Step::Done, OutputItem::Abort),
            ClearLine => {
                terminal.move_cursor_to_start_of_line();

                self.transition(Step::Done, OutputItem::Slice("\r\x1b[J".as_bytes()))
            }
            GetPosition => self.transition(Step::Done, OutputItem::Slice("\x1b[6n".as_bytes())),
            SavePosition => self.transition(Step::Done, OutputItem::Slice(b"\x1b7")),
            RestorePosition => self.transition(Step::Done, OutputItem::Slice(b"\x1b8")),
            Done => None,
        }
    }
}
