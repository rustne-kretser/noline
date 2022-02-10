use core::array::IntoIter;

use crate::{
    line_buffer::{Buffer, LineBuffer},
    terminal::{Cursor, Position, Terminal},
};

pub enum OutputItem<'a> {
    Slice(&'a [u8]),
    UintToBytes(UintToBytes<4>),
    EndOfString,
    Abort,
}

impl<'a> OutputItem<'a> {
    pub fn get_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Slice(slice) => Some(slice),
            Self::UintToBytes(uint) => Some(uint.as_bytes()),
            Self::EndOfString | Self::Abort => None,
        }
    }
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
    PrintBufferAndMoveCursorForward,
    EraseAfterCursor,
    EraseAndPrintBuffer,
    ClearScreen,
    ClearLine,
    MoveCursorBackAndPrintBufferAndMoveForward,
    MoveCursorAndEraseAndPrintBuffer(isize),
    RingBell,
    PrintNewline,
    Done,
    Abort,
}

#[cfg_attr(test, derive(Debug))]
#[derive(Copy, Clone)]
pub struct UintToBytes<const N: usize> {
    bytes: [u8; N],
}

impl<const N: usize> UintToBytes<N> {
    fn from_uint<I: Into<usize>>(n: I) -> Option<Self> {
        let mut n: usize = n.into();

        if n < 10_usize.pow(N as u32) {
            let mut bytes = [0; N];

            for i in (0..N).rev() {
                bytes[i] = 0x30 + (n % 10) as u8;
                n = n / 10;

                if n == 0 {
                    break;
                }
            }

            Some(Self { bytes })
        } else {
            None
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        let start = self.bytes.iter().take_while(|&&b| b == 0).count();
        &self.bytes[start..]
    }
}

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
struct MoveCursor {
    state: MoveCursorState,
    cursor: Cursor,
    scroll: isize,
}

impl MoveCursor {
    fn new(cursor: Cursor, scroll: isize) -> Self {
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
                        UintToBytes::from_uint(self.scroll.abs() as usize).unwrap(),
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

#[cfg_attr(test, derive(Debug))]
enum MoveCursorToPosition {
    Position(Position),
    Move(MoveCursor),
}

impl MoveCursorToPosition {
    fn new(position: Position) -> Self {
        Self::Position(position)
    }

    fn get_move_cursor(&mut self, terminal: &mut Terminal) -> Option<&mut MoveCursor> {
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

#[cfg_attr(test, derive(Debug))]
enum Step<'a> {
    Print(&'a str),
    NewlinePrint(&'a str),
    Move(MoveCursorToPosition),
    GetPosition,
    ClearLine,
    Erase,
    Newline,
    Bell,
    EndOfString,
    Abort,
    Done,
}

impl<'a> Step<'a> {
    fn transition(
        &mut self,
        new_state: Step<'a>,
        output: OutputItem<'a>,
    ) -> Option<OutputItem<'a>> {
        *self = new_state;
        Some(output)
    }

    fn advance(&mut self, terminal: &mut Terminal) -> Option<OutputItem<'a>> {
        match self {
            Print(s) => {
                let columns_remaining = terminal.columns_remaining();

                let (s, rest) = s.split_at(columns_remaining.min(s.len()));

                #[cfg(test)]
                dbg!(s, rest);

                let position = terminal.relative_position(s.chars().count() as isize);
                terminal.move_cursor(position);

                let step = if s.len() == columns_remaining {
                    Step::NewlinePrint(rest)
                } else {
                    Step::Done
                };

                self.transition(step, OutputItem::Slice(s.as_bytes()))
            }
            NewlinePrint(s) => {
                let step = Step::Print(s);
                self.transition(step, OutputItem::Slice("\n\r".as_bytes()))
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
            Erase => self.transition(Step::Done, OutputItem::Slice("\x1b[J".as_bytes())),
            Newline => self.transition(Step::Done, OutputItem::Slice("\n\r".as_bytes())),
            Bell => self.transition(Step::Done, OutputItem::Slice("\x07".as_bytes())),
            EndOfString => self.transition(Step::Done, OutputItem::EndOfString),
            Abort => self.transition(Step::Done, OutputItem::Abort),
            ClearLine => {
                terminal.move_cursor_to_start_of_line();

                self.transition(Step::Done, OutputItem::Slice("\r\x1b[J".as_bytes()))
            }
            GetPosition => self.transition(Step::Done, OutputItem::Slice("\x1b[6n".as_bytes())),
            Done => None,
        }
    }
}

use Step::*;

#[cfg_attr(test, derive(Debug))]
enum OutputState<'a> {
    New(OutputAction),
    OneStep(IntoIter<Step<'a>, 1>),
    TwoSteps(IntoIter<Step<'a>, 2>),
    ThreeSteps(IntoIter<Step<'a>, 3>),
    FourSteps(IntoIter<Step<'a>, 4>),
    Done,
}

fn byte_position(s: &str, char_pos: usize) -> usize {
    s.char_indices()
        .skip(char_pos)
        .map(|(pos, _)| pos)
        .next()
        .unwrap_or(s.len())
}

pub struct Output<'a, B: Buffer> {
    prompt: &'a str,
    buffer: &'a LineBuffer<B>,
    terminal: &'a mut Terminal,
    state: OutputState<'a>,
}

impl<'a, B: Buffer> Output<'a, B> {
    pub fn new(
        prompt: &'a str,
        buffer: &'a LineBuffer<B>,
        terminal: &'a mut Terminal,
        action: OutputAction,
    ) -> Self {
        Self {
            prompt,
            buffer,
            terminal,
            state: OutputState::New(action),
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

        #[cfg(test)]
        dbg!(self.terminal.current_offset(), self.prompt.len(), s, pos);

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
}

impl<'a, B: Buffer> Iterator for Output<'a, B> {
    type Item = OutputItem<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        fn advance_steps<'a, const N: usize>(
            steps: &mut IntoIter<Step<'a>, N>,
            terminal: &mut Terminal,
        ) -> Option<OutputItem<'a>> {
            loop {
                if let Some((step, _)) = steps.as_mut_slice().split_first_mut() {
                    #[cfg(test)]
                    dbg!(&step);

                    if let Some(bytes) = step.advance(terminal) {
                        break Some(bytes);
                    } else {
                        steps.next();
                    }
                } else {
                    break None;
                }
            }
        }

        loop {
            // #[cfg(test)]
            // dbg!(&self.state);

            match self.state {
                OutputState::New(action) => {
                    self.state = match action {
                        OutputAction::MoveCursor(cursor_move) => {
                            let position = self.new_position(cursor_move);

                            let offset = self.terminal.offset_from_position(position)
                                - self.prompt.len() as isize;
                            let buffer_len = self.buffer.as_str().chars().count() as isize;

                            if offset >= 0 && offset <= buffer_len {
                                OutputState::OneStep(
                                    [Move(MoveCursorToPosition::new(
                                        self.new_position(cursor_move),
                                    ))]
                                    .into_iter(),
                                )
                            } else {
                                OutputState::OneStep([Bell].into_iter())
                            }
                        }
                        OutputAction::PrintBufferAndMoveCursorForward => OutputState::TwoSteps(
                            [
                                Print(self.buffer_after_position(self.terminal.get_position())),
                                Move(MoveCursorToPosition::new(
                                    self.terminal.relative_position(1),
                                )),
                            ]
                            .into_iter(),
                        ),
                        OutputAction::EraseAfterCursor => OutputState::OneStep([Erase].into_iter()),
                        OutputAction::EraseAndPrintBuffer => {
                            let position = self.terminal.get_position();

                            OutputState::ThreeSteps(
                                [
                                    Erase,
                                    Print(self.buffer_after_position(position)),
                                    Move(MoveCursorToPosition::new(position)),
                                ]
                                .into_iter(),
                            )
                        }

                        OutputAction::ClearScreen => {
                            let rows = self.terminal.scroll_to_top();
                            self.terminal.move_cursor(Position::new(0, 0));

                            OutputState::ThreeSteps(
                                [
                                    Move(MoveCursorToPosition::Move(MoveCursor::new(
                                        Cursor::new(0, 0),
                                        rows,
                                    ))),
                                    Erase,
                                    Print(self.prompt),
                                ]
                                .into_iter(),
                            )
                        }
                        OutputAction::ClearLine => OutputState::TwoSteps(
                            [
                                Move(MoveCursorToPosition::new(
                                    self.new_position(CursorMove::Start),
                                )),
                                Erase,
                            ]
                            .into_iter(),
                        ),
                        OutputAction::MoveCursorBackAndPrintBufferAndMoveForward => {
                            let position = self.terminal.relative_position(-1);

                            OutputState::ThreeSteps(
                                [
                                    Move(MoveCursorToPosition::new(position)),
                                    Print(self.buffer_after_position(position)),
                                    Move(MoveCursorToPosition::new(self.terminal.get_position())),
                                ]
                                .into_iter(),
                            )
                        }
                        OutputAction::MoveCursorAndEraseAndPrintBuffer(steps) => {
                            let position = self.terminal.relative_position(steps);

                            OutputState::FourSteps(
                                [
                                    Move(MoveCursorToPosition::new(position)),
                                    Erase,
                                    Print(self.buffer_after_position(position)),
                                    Move(MoveCursorToPosition::new(position)),
                                ]
                                .into_iter(),
                            )
                        }
                        OutputAction::RingBell => OutputState::OneStep([Bell].into_iter()),
                        OutputAction::PrintNewline => OutputState::OneStep([Newline].into_iter()),
                        OutputAction::ClearAndPrintPrompt => OutputState::ThreeSteps(
                            [ClearLine, Print(self.prompt), GetPosition].into_iter(),
                        ),
                        OutputAction::Done => {
                            OutputState::TwoSteps([Newline, EndOfString].into_iter())
                        }
                        OutputAction::Abort => OutputState::TwoSteps([Newline, Abort].into_iter()),
                        OutputAction::Nothing => OutputState::Done,
                    };

                    continue;
                }
                OutputState::OneStep(ref mut steps) => {
                    if let Some(bytes) = advance_steps(steps, self.terminal) {
                        break Some(bytes);
                    } else {
                        self.state = OutputState::Done;
                        continue;
                    }
                }
                OutputState::TwoSteps(ref mut steps) => {
                    if let Some(bytes) = advance_steps(steps, self.terminal) {
                        break Some(bytes);
                    } else {
                        self.state = OutputState::Done;
                        continue;
                    }
                }
                OutputState::ThreeSteps(ref mut steps) => {
                    if let Some(bytes) = advance_steps(steps, self.terminal) {
                        break Some(bytes);
                    } else {
                        self.state = OutputState::Done;
                        continue;
                    }
                }
                OutputState::FourSteps(ref mut steps) => {
                    if let Some(bytes) = advance_steps(steps, self.terminal) {
                        break Some(bytes);
                    } else {
                        self.state = OutputState::Done;
                        continue;
                    }
                }
                OutputState::Done => break None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::string::String;

    use crate::line_buffer::AllocLineBuffer;

    use super::*;

    use std::vec::Vec;

    #[test]
    fn uint_to_bytes() {
        fn to_string<const N: usize>(n: usize) -> String {
            let uint: UintToBytes<N> = UintToBytes::from_uint(n).unwrap();

            String::from_utf8(uint.as_bytes().iter().map(|&b| b).collect::<Vec<u8>>()).unwrap()
        }

        assert_eq!(to_string::<4>(0), "0");

        assert_eq!(to_string::<4>(42), "42");

        assert_eq!(to_string::<4>(10), "10");

        assert_eq!(to_string::<4>(9999), "9999");
    }

    #[test]
    fn move_cursor() {
        fn to_string(cm: MoveCursor) -> String {
            String::from_utf8(
                cm.map(|item| {
                    if let Some(bytes) = item.get_bytes() {
                        bytes.iter().map(|&b| b).collect::<Vec<u8>>()
                    } else {
                        vec![]
                    }
                })
                .flatten()
                .collect(),
            )
            .unwrap()
        }

        assert_eq!(
            to_string(MoveCursor::new(Cursor::new(42, 0), 0)),
            "\x1b[43;1H"
        );

        assert_eq!(
            to_string(MoveCursor::new(Cursor::new(0, 42), 0)),
            "\x1b[1;43H"
        );

        assert_eq!(
            to_string(MoveCursor::new(Cursor::new(42, 43), 0)),
            "\x1b[43;44H"
        );

        assert_eq!(
            to_string(MoveCursor::new(Cursor::new(0, 0), 0)),
            "\x1b[1;1H"
        );

        assert_eq!(
            to_string(MoveCursor::new(Cursor::new(0, 9), 0)),
            "\x1b[1;10H"
        );

        assert_eq!(
            to_string(MoveCursor::new(Cursor::new(0, 0), 1)),
            "\x1b[1S\x1b[1;1H"
        );

        assert_eq!(
            to_string(MoveCursor::new(Cursor::new(0, 0), -1)),
            "\x1b[1T\x1b[1;1H"
        );
    }

    #[test]
    fn step() {
        fn to_string(mut step: Step, terminal: &mut Terminal) -> String {
            let mut bytes = Vec::new();

            while let Some(item) = step.advance(terminal) {
                if let Some(slice) = item.get_bytes() {
                    for b in slice {
                        bytes.push(*b);
                    }
                }
            }

            String::from_utf8(bytes).unwrap()
        }

        let mut terminal = Terminal::new(4, 10, Cursor::new(0, 0));

        assert_eq!(
            to_string(Step::Print("01234567890123456789"), &mut terminal),
            "0123456789\n\r0123456789\n\r"
        );

        assert_eq!(to_string(Step::Print("01234"), &mut terminal), "01234");

        assert_eq!(
            to_string(Step::Print("5678901234567890"), &mut terminal),
            "56789\n\r0123456789\n\r0"
        );

        assert_eq!(terminal.get_position(), Position::new(4, 1));

        assert_eq!(
            to_string(
                Step::Move(MoveCursorToPosition::new(Position::new(0, 3))),
                &mut terminal
            ),
            "\x1b[1T\x1b[1;4H"
        );

        assert_eq!(terminal.get_position(), Position::new(0, 3));

        assert_eq!(to_string(Step::Erase, &mut terminal), "\x1b[J");
        assert_eq!(to_string(Step::Newline, &mut terminal), "\n\r");
        assert_eq!(to_string(Step::Bell, &mut terminal), "\x07");
        assert_eq!(to_string(Step::Done, &mut terminal), "");
    }

    #[test]
    fn byte_iterator() {
        fn to_string<'a, B: Buffer>(output: Output<'a, B>) -> String {
            String::from_utf8(
                output
                    .map(|item| {
                        if let Some(bytes) = item.get_bytes() {
                            bytes.iter().map(|&b| b).collect::<Vec<u8>>()
                        } else {
                            vec![]
                        }
                    })
                    .flatten()
                    .collect(),
            )
            .unwrap()
        }

        let prompt = "> ";
        let mut line_buffer = AllocLineBuffer::new();
        let mut terminal = Terminal::new(4, 10, Cursor::new(0, 0));

        let result = to_string(Output::new(
            prompt,
            &line_buffer,
            &mut terminal,
            OutputAction::ClearAndPrintPrompt,
        ));

        assert_eq!(result, "\r\x1b[J> \x1b[6n");

        line_buffer.insert_str(0, "Hello, world!");

        let result = to_string(Output::new(
            prompt,
            &line_buffer,
            &mut terminal,
            OutputAction::PrintBufferAndMoveCursorForward,
        ));

        assert_eq!(result, "Hello, w\n\rorld!\x1b[1;4H");

        assert_eq!(terminal.get_cursor(), Cursor::new(0, 3));

        let result = to_string(Output::new(
            prompt,
            &line_buffer,
            &mut terminal,
            OutputAction::MoveCursor(CursorMove::Start),
        ));

        assert_eq!(result, "\x1b[1;3H");
        assert_eq!(terminal.get_cursor(), Cursor::new(0, 2));
    }
}
