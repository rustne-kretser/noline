use super::{
    move_cursor::MoveCursor, move_cursor_pos::MoveCursorToPosition, printable::Printable,
    step::Step::*, OutputItem, OutputIter,
};
use crate::{
    core::Prompt,
    line_buffer::{Buffer, LineBuffer},
    terminal::{Cursor, Position, Terminal},
};
use core::marker::PhantomData;

fn byte_position(s: &str, char_pos: usize) -> usize {
    s.char_indices()
        .skip(char_pos)
        .map(|(pos, _)| pos)
        .next()
        .unwrap_or(s.len())
}

#[cfg_attr(test, derive(Debug))]
#[derive(Copy, Clone)]
pub enum CursorMove {
    Forward,
    Back,
    Start,
    End,
}

#[cfg_attr(test, derive(Debug))]
#[derive(Copy, Clone)]
pub enum OutputAction {
    Nothing,
    MoveCursor(CursorMove),
    ClearAndPrintPrompt,
    ClearAndPrintBuffer,
    PrintBufferAndMoveCursorForward,
    EraseAfterCursor,
    EraseAndPrintBuffer,
    ClearScreen,
    ClearLine,
    MoveCursorBackAndPrintBufferAndMoveForward,
    MoveCursorAndEraseAndPrintBuffer(isize),
    RingBell,
    ProbeSize,
    Done,
    Abort,
}

pub struct Output<'a, B: Buffer, I> {
    prompt: &'a Prompt<I>,
    buffer: &'a LineBuffer<B>,
    terminal: &'a mut Terminal,
    action: OutputAction,
}

impl<'a, 'item, B, I> Output<'a, B, I>
where
    B: Buffer,
    I: Iterator<Item = &'item str> + Clone,
{
    pub fn new(
        prompt: &'a Prompt<I>,
        buffer: &'a LineBuffer<B>,
        terminal: &'a mut Terminal,
        action: OutputAction,
    ) -> Self {
        Self {
            prompt,
            buffer,
            terminal,
            action,
        }
    }

    fn offset_from_position(&self, position: Position) -> usize {
        self.terminal.offset_from_position(position) as usize - self.prompt.len()
    }

    fn current_offset(&self) -> usize {
        self.offset_from_position(self.terminal.get_position())
    }

    fn buffer_after_position(&self, position: Position) -> &'a str {
        let offset = self.offset_from_position(position);
        let s = self.buffer.as_str();

        let pos = byte_position(s, offset);

        &s[pos..]
    }

    fn new_position(&self, cursor_move: CursorMove) -> Position {
        match cursor_move {
            CursorMove::Forward => self.terminal.relative_position(1),
            CursorMove::Back => self.terminal.relative_position(-1),
            CursorMove::Start => {
                let pos = self.current_offset() as isize;
                self.terminal.relative_position(-pos)
            }
            CursorMove::End => {
                let pos = self.current_offset() as isize;
                let len = self.buffer.as_str().chars().count() as isize;
                #[cfg(test)]
                dbg!(pos, len);
                self.terminal.relative_position(len - pos)
            }
        }
    }

    #[cfg(test)]
    pub fn into_vec(self) -> Vec<u8> {
        self.into_iter()
            .flat_map(|item| item.get_bytes().unwrap().to_vec())
            .collect::<Vec<u8>>()
    }
}

impl<'a, 'item, B, I> IntoIterator for Output<'a, B, I>
where
    B: Buffer,
    I: Iterator<Item = &'item str> + Clone,
    'item: 'a,
{
    type Item = OutputItem<'a>;
    type IntoIter = OutputIter<'a, 'item, I>;

    fn into_iter(self) -> Self::IntoIter {
        fn pack<T, const IN: usize, const OUT: usize>(array: [T; IN]) -> [Option<T>; OUT] {
            const {
                assert!(IN <= OUT);
            }

            let mut steps = [(); OUT].map(|()| None);

            for (i, step) in array.into_iter().enumerate() {
                steps[i] = Some(step);
            }

            steps
        }

        let steps = match self.action {
            OutputAction::MoveCursor(cursor_move) => {
                let position = self.new_position(cursor_move);

                let offset =
                    self.terminal.offset_from_position(position) - self.prompt.len() as isize;
                let buffer_len = self.buffer.as_str().chars().count() as isize;

                if offset >= 0 && offset <= buffer_len {
                    pack([Move(MoveCursorToPosition::new(
                        self.new_position(cursor_move),
                    ))])
                } else {
                    pack([Bell])
                }
            }
            OutputAction::PrintBufferAndMoveCursorForward => pack([
                Print(Printable::from_str(
                    self.buffer_after_position(self.terminal.get_position()),
                )),
                Move(MoveCursorToPosition::new(
                    self.terminal.relative_position(1),
                )),
            ]),
            OutputAction::EraseAfterCursor => pack([Erase]),
            OutputAction::EraseAndPrintBuffer => {
                let position = self.terminal.get_position();

                pack([
                    Erase,
                    Print(Printable::from_str(self.buffer_after_position(position))),
                    Move(MoveCursorToPosition::new(position)),
                ])
            }

            OutputAction::ClearScreen => {
                let rows = self.terminal.scroll_to_top();
                self.terminal.move_cursor(Position::new(0, 0));

                pack([
                    Move(MoveCursorToPosition::Move(MoveCursor::new(
                        Cursor::new(0, 0),
                        rows,
                    ))),
                    Erase,
                    Print(Printable::from_iter(self.prompt.iter())),
                ])
            }
            OutputAction::ClearLine => pack([
                Move(MoveCursorToPosition::new(
                    self.new_position(CursorMove::Start),
                )),
                Erase,
            ]),
            OutputAction::MoveCursorBackAndPrintBufferAndMoveForward => {
                let position = self.terminal.relative_position(-1);

                pack([
                    Move(MoveCursorToPosition::new(position)),
                    Print(Printable::from_str(self.buffer_after_position(position))),
                    Move(MoveCursorToPosition::new(self.terminal.get_position())),
                ])
            }
            OutputAction::MoveCursorAndEraseAndPrintBuffer(steps) => {
                let position = self.terminal.relative_position(steps);

                pack([
                    Move(MoveCursorToPosition::new(position)),
                    Erase,
                    Print(Printable::from_str(self.buffer_after_position(position))),
                    Move(MoveCursorToPosition::new(position)),
                ])
            }
            OutputAction::RingBell => pack([Bell]),
            OutputAction::ClearAndPrintPrompt => pack([
                ClearLine,
                Print(Printable::from_iter(self.prompt.iter())),
                GetPosition,
            ]),
            OutputAction::ClearAndPrintBuffer => {
                let position = self.new_position(CursorMove::Start);

                pack([
                    Move(MoveCursorToPosition::new(position)),
                    Erase,
                    Print(Printable::from_str(self.buffer.as_str())),
                ])
            }
            OutputAction::ProbeSize => {
                pack([SavePosition, MoveCursorToEdge, GetPosition, RestorePosition])
            }

            OutputAction::Done => pack([Newline, EndOfString]),
            OutputAction::Abort => pack([Newline, Abort]),
            OutputAction::Nothing => pack([]),
        };

        OutputIter {
            terminal: self.terminal,
            steps,
            pos: 0,
            _marker: PhantomData,
        }
    }
}
