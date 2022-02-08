use core::marker::PhantomData;

use crate::input::{Action, ControlCharacter::*, Parser, CSI};
use crate::line_buffer::Buffer;
use crate::line_buffer::LineBuffer;
use crate::marker::SyncAsync;
use crate::output::CursorMove;
use crate::output::{Output, OutputAction};
use crate::terminal::{Cursor, Terminal};

use OutputAction::*;

pub enum InitializerResult<T> {
    Continue,
    Item(T),
    InvalidInput,
}

#[cfg_attr(test, derive(Debug, PartialEq, Eq))]
pub(crate) enum NolineInitializerState {
    New,
    Position(Cursor),
    Done,
}

pub struct NolineInitializer<'a, B: Buffer, S: SyncAsync> {
    pub(crate) state: NolineInitializerState,
    parser: Parser,
    pub(crate) buffer: &'a mut LineBuffer<B>,
    pub(crate) prompt: &'a str,
    _marker: PhantomData<S>,
}

impl<'a, B: Buffer, S: SyncAsync> NolineInitializer<'a, B, S> {
    pub fn new(buffer: &'a mut LineBuffer<B>, prompt: &'a str) -> Self {
        Self {
            state: NolineInitializerState::New,
            parser: Parser::new(),
            buffer,
            prompt,
            _marker: PhantomData,
        }
    }

    pub(crate) fn clear_line(&self) -> &'static [u8] {
        "\r\x1b[J".as_bytes()
    }

    pub(crate) fn probe_size(&self) -> &'static [u8] {
        "\x1b7\x1b[6n\x1b[999;999H\x1b[6n\x1b8".as_bytes()
    }

    pub(crate) fn advance(&mut self, byte: u8) -> InitializerResult<Terminal> {
        let action = self.parser.advance(byte);

        #[cfg(test)]
        dbg!(byte, action, &self.state);

        match action {
            Action::ControlSequenceIntroducer(CSI::CPR(x, y)) => match self.state {
                NolineInitializerState::New => {
                    self.state = NolineInitializerState::Position(Cursor::new(x - 1, y - 1));
                    InitializerResult::Continue
                }
                NolineInitializerState::Position(pos) => {
                    #[cfg(test)]
                    dbg!(pos, x, y);

                    self.state = NolineInitializerState::Done;
                    InitializerResult::Item(Terminal::new(x, y, pos))
                }
                NolineInitializerState::Done => InitializerResult::InvalidInput,
            },
            Action::Ignore => InitializerResult::Continue,
            _ => InitializerResult::InvalidInput,
        }
    }
}

pub struct Noline<'a, B: Buffer, S: SyncAsync> {
    pub(crate) buffer: &'a mut LineBuffer<B>,
    terminal: Terminal,
    parser: Parser,
    prompt: &'a str,
    _marker: PhantomData<S>,
}

impl<'a, B: Buffer, S: SyncAsync> Noline<'a, B, S> {
    pub fn new(line_buffer: &'a mut LineBuffer<B>, prompt: &'a str, terminal: Terminal) -> Self {
        Self {
            buffer: line_buffer,
            terminal,
            parser: Parser::new(),
            prompt,
            _marker: PhantomData,
        }
    }

    fn output<'b>(&'b mut self, action: OutputAction) -> Output<'b, B> {
        Output::new(self.prompt, &*self.buffer, &mut self.terminal, action)
    }

    pub fn print_prompt<'b>(&'b mut self) -> Output<'b, B> {
        self.output(PrintPrompt)
    }

    fn current_position(&self) -> usize {
        let pos = self.terminal.current_offset() as usize;
        pos - self.prompt.len()
    }

    pub(crate) fn input_byte<'b>(&'b mut self, byte: u8) -> Output<'b, B> {
        let action = self.parser.advance(byte);

        #[cfg(test)]
        dbg!(action);

        match action {
            Action::Print(c) => {
                let pos = self.current_position();

                if self.buffer.insert_utf8_char(pos, c).is_ok() {
                    self.output(PrintBufferAndMoveCursorForward)
                } else {
                    self.output(RingBell)
                }
            }
            Action::ControlCharacter(c) => match c {
                CtrlA => self.output(MoveCursor(CursorMove::Start)),
                CtrlB => self.output(MoveCursor(CursorMove::Back)),
                CtrlC => self.output(Abort),
                CtrlD => {
                    let len = self.buffer.len();

                    if len > 0 {
                        let pos = self.current_position();

                        if pos < len {
                            self.buffer.delete(pos);

                            self.output(EraseAndPrintBuffer)
                        } else {
                            self.output(RingBell)
                        }
                    } else {
                        self.output(Abort)
                    }
                }
                CtrlE => self.output(MoveCursor(CursorMove::End)),
                CtrlF => self.output(MoveCursor(CursorMove::Forward)),
                CtrlK => {
                    let pos = self.current_position();

                    self.buffer.delete_after_char(pos);

                    self.output(EraseAfterCursor)
                }
                CtrlL => {
                    self.buffer.delete_after_char(0);
                    self.output(ClearScreen)
                }
                CtrlT => {
                    let pos = self.current_position();

                    if pos > 0 && pos < self.buffer.as_str().chars().count() {
                        self.buffer.swap_chars(pos);
                        self.output(MoveCursorBackAndPrintBufferAndMoveForward)
                    } else {
                        self.output(RingBell)
                    }
                }
                CtrlU => {
                    self.buffer.delete_after_char(0);
                    self.output(ClearLine)
                }
                CtrlW => {
                    let pos = self.current_position();
                    let move_cursor = -(self.buffer.delete_previous_word(pos) as isize);
                    self.output(MoveCursorAndEraseAndPrintBuffer(move_cursor))
                }
                CarriageReturn => self.output(Done),
                CtrlH | Backspace => {
                    let pos = self.current_position();
                    if pos > 0 {
                        self.buffer.delete(pos - 1);
                        self.output(MoveCursorAndEraseAndPrintBuffer(-1))
                    } else {
                        self.output(RingBell)
                    }
                }
                _ => self.output(RingBell),
            },
            Action::ControlSequenceIntroducer(csi) => match csi {
                CSI::CUF(_) => self.output(MoveCursor(CursorMove::Forward)),
                CSI::CUB(_) => self.output(MoveCursor(CursorMove::Back)),
                CSI::Home => self.output(MoveCursor(CursorMove::Start)),
                CSI::Delete => {
                    let len = self.buffer.len();
                    let pos = self.current_position();

                    if pos < len {
                        self.buffer.delete(pos);

                        self.output(EraseAndPrintBuffer)
                    } else {
                        self.output(RingBell)
                    }
                }
                CSI::End => self.output(MoveCursor(CursorMove::End)),
                CSI::Unknown(_) => self.output(RingBell),
                CSI::CUU(_) => self.output(RingBell),
                CSI::CUD(_) => self.output(RingBell),
                CSI::CPR(_, _) => self.output(RingBell),
                CSI::CUP(_, _) => self.output(RingBell),
                CSI::ED(_) => self.output(RingBell),
                CSI::DSR => self.output(RingBell),
                CSI::SU(_) => self.output(RingBell),
                CSI::SD(_) => self.output(RingBell),
            },
            Action::EscapeSequence(_) => self.output(RingBell),
            Action::Ignore => self.output(Nothing),
            Action::InvalidUtf8 => self.output(RingBell),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::channel;
    use std::vec::Vec;

    use std::string::String;

    use std::boxed::Box;

    use crate::error::Error;
    use crate::input::tests::AsByteVec;
    use crate::line_buffer::AllocLineBuffer;
    use crate::sync::{Noline, NolineInitializer};
    use crate::terminal::Cursor;

    use super::*;

    struct MockTerminal<'a, B: Buffer> {
        noline: Option<Noline<'a, B>>,
        parser: Parser,
        screen: Vec<Vec<char>>,
        cursor: Cursor,
        rows: usize,
        columns: usize,
        saved_cursor: Option<Cursor>,
        bell: bool,
    }

    impl<'a, B: Buffer> MockTerminal<'a, B> {
        fn new(
            buffer: &'a mut LineBuffer<B>,
            prompt: &'a str,
            rows: usize,
            columns: usize,
            origin: Cursor,
        ) -> Self {
            let mut term = Self {
                noline: None, // Some(noline),
                parser: Parser::new(),
                screen: vec![vec!['\0'; columns]; rows],
                cursor: origin,
                rows,
                columns,
                saved_cursor: None,
                bell: false,
            };

            let (tx, rx) = channel();

            let noline = NolineInitializer::new(buffer, prompt)
                .initialize(
                    || {
                        if let Ok(b) = rx.try_recv() {
                            Ok(b)
                        } else {
                            Err(Error::IoError(()))
                        }
                    },
                    |bytes| {
                        for byte in bytes {
                            if let Some(bytes) = term.handle_input(*byte) {
                                for byte in bytes {
                                    tx.send(byte).unwrap();
                                }
                            }
                        }

                        Ok(())
                    },
                )
                .unwrap();

            assert!(rx.try_recv().is_err());

            term.noline = Some(noline);
            term
        }

        fn current_line(&mut self) -> &mut Vec<char> {
            let cursor = self.get_cursor();

            &mut self.screen[cursor.row as usize]
        }

        fn screen_as_string(&self) -> String {
            self.screen
                .iter()
                .map(|v| v.iter().take_while(|&&c| c != '\0').collect::<String>())
                .filter(|s| s.len() > 0)
                .collect::<Vec<String>>()
                .join("\n")
        }

        fn buffer_as_str(&self) -> &str {
            self.noline.as_ref().unwrap().buffer.as_str()
        }

        fn move_column(&mut self, steps: isize) {
            self.cursor.column = 0
                .max((self.cursor.column as isize + steps).min(self.columns as isize - 1))
                as usize;
            dbg!(self.cursor.column);
        }

        fn scroll_up(&mut self, lines: usize) {
            for _ in 0..lines {
                self.screen.remove(0);
                self.screen.push(vec!['\0'; self.columns]);
            }
        }

        fn scroll_down(&mut self, lines: usize) {
            for _ in 0..lines {
                self.screen.pop();
                self.screen.insert(0, vec!['\0'; self.columns]);
            }
        }

        fn handle_input(&mut self, byte: u8) -> Option<Vec<u8>> {
            let mock_term_action = self.parser.advance(byte);

            dbg!(mock_term_action);
            match mock_term_action {
                Action::Ignore => (),
                Action::Print(c) => {
                    let pos = self.cursor.column;
                    let line = self.current_line();

                    line[pos] = c.to_char();
                    self.move_column(1);
                }
                Action::ControlSequenceIntroducer(csi) => {
                    match csi {
                        CSI::CUU(_) => unimplemented!(),
                        CSI::CUD(_) => unimplemented!(),
                        CSI::CUF(_) => unimplemented!(),
                        CSI::CUB(_) => unimplemented!(),
                        CSI::CPR(_, _) => unimplemented!(),
                        CSI::CUP(row, column) => {
                            self.cursor = Cursor::new(
                                (row - 1).min(self.rows - 1),
                                (column - 1).min(self.columns - 1),
                            );
                        }
                        CSI::ED(_) => {
                            let cursor = self.get_cursor();

                            for row in (cursor.row as usize)..self.rows {
                                let start = if row == cursor.row as usize {
                                    cursor.column as usize
                                } else {
                                    0
                                };
                                for column in (start)..self.columns {
                                    self.screen[row][column] = '\0';
                                }
                            }
                        }
                        CSI::DSR => {
                            return Some(format!("\x1b[{};{}R", self.cursor.row + 1, self.cursor.column + 1,)
                                .bytes()
                                .collect::<Vec<u8>>());
                        }
                        CSI::Unknown(b) => {
                            dbg!(b as char);
                            unimplemented!()
                        }
                        CSI::SU(lines) => {
                            self.scroll_up(lines);
                        }
                        CSI::SD(lines) => {
                            self.scroll_down(lines);
                        }
                        CSI::Home => unimplemented!(),
                        CSI::Delete => unimplemented!(),
                        CSI::End => unimplemented!(),
                    }
                }
                Action::InvalidUtf8 => unreachable!(),
                Action::ControlCharacter(ctrl) => {
                    dbg!(ctrl);

                    match ctrl {
                        CarriageReturn => self.cursor.column = 0,
                        LineFeed => {
                            if self.cursor.row + 1 == self.rows {
                                self.scroll_up(1);
                            } else {
                                self.cursor.row += 1;
                            }
                        }
                        CtrlG => self.bell = true,
                        _ => (),
                    }
                }
                Action::EscapeSequence(esc) => match esc {
                    0x37 => {
                        self.saved_cursor = Some(self.cursor);
                    }
                    0x38 => {
                        let cursor = self.saved_cursor.unwrap();
                        self.cursor = cursor;
                    }
                    _ => {
                        dbg!(esc);
                    }
                },
            }

            None
        }

        fn advance(&mut self, input: impl AsByteVec) -> core::result::Result<(), ()> {
            self.bell = false;
            let mut noline = self.noline.take().unwrap();

            for input in input.as_byte_vec() {
                noline.advance::<_, ()>(input, |bytes| {
                    for output in bytes {
                        self.handle_input(*output);
                    }
                    Ok(())
                });
            }

            assert_eq!(noline.terminal.get_cursor(), self.cursor);

            self.noline = Some(noline);

            dbg!(self.screen_as_string());

            if self.bell {
                Err(())
            } else {
                Ok(())
            }
        }

        fn get_cursor(&self) -> Cursor {
            self.cursor
        }
    }

    const LEFT: &str = "\x1b[D";
    const RIGHT: &str = "\x1b[C";
    const HOME: &str = "\x1b[1~";
    const DELETE: &str = "\x1b[3~";
    const END: &str = "\x1b[4~";

    fn get_terminal<'a>(
        prompt: &'a str,
        rows: usize,
        columns: usize,
        origin: Cursor,
    ) -> MockTerminal<'a, Vec<u8>> {
        let buffer = Box::leak(Box::new(AllocLineBuffer::new()));
        let terminal = MockTerminal::new(buffer, prompt, rows, columns, origin);

        assert_eq!(terminal.get_cursor(), Cursor::new(origin.row, 2));
        assert_eq!(terminal.screen_as_string(), prompt);

        terminal
    }

    #[test]
    fn movecursor() {
        let prompt = "> ";
        let mut terminal = get_terminal(prompt, 4, 10, Cursor::new(1, 0));

        terminal.advance("Hello, World!").unwrap();
        assert_eq!(terminal.get_cursor(), Cursor::new(2, 5));

        terminal.advance([LEFT; 6]).unwrap();
        assert_eq!(terminal.get_cursor(), Cursor::new(1, 9));

        terminal.advance(CtrlA).unwrap();

        assert_eq!(terminal.get_cursor(), Cursor::new(1, 2));

        assert!(terminal.advance(LEFT).is_err());

        assert_eq!(terminal.get_cursor(), Cursor::new(1, 2));

        terminal.advance(CtrlE).unwrap();

        assert_eq!(terminal.get_cursor(), Cursor::new(2, 5));

        assert!(terminal.advance(RIGHT).is_err());

        assert_eq!(terminal.get_cursor(), Cursor::new(2, 5));

        terminal.advance(HOME).unwrap();

        assert_eq!(terminal.get_cursor(), Cursor::new(1, 2));

        terminal.advance(END).unwrap();

        assert_eq!(terminal.get_cursor(), Cursor::new(2, 5));
    }

    #[test]
    fn cursor_scroll() {
        let prompt = "> ";
        let mut terminal = get_terminal(prompt, 4, 10, Cursor::new(3, 0));

        terminal.advance("23456789").unwrap();
        assert_eq!(terminal.get_cursor(), Cursor::new(3, 0));
    }

    #[test]
    fn clear_line() {
        let prompt = "> ";
        let mut terminal = get_terminal(prompt, 4, 20, Cursor::new(1, 0));

        terminal.advance("Hello, World!").unwrap();
        assert_eq!(terminal.get_cursor(), Cursor::new(1, 15));
        assert_eq!(terminal.screen_as_string(), "> Hello, World!");

        terminal.advance(CtrlU).unwrap();
        assert_eq!(terminal.get_cursor(), Cursor::new(1, 2));
        assert_eq!(terminal.screen_as_string(), "> ");
    }

    #[test]
    fn clear_screen() {
        let prompt = "> ";
        let mut terminal = get_terminal(prompt, 4, 20, Cursor::new(1, 0));

        terminal.advance("Hello, World!").unwrap();
        assert_eq!(terminal.get_cursor(), Cursor::new(1, 15));
        assert_eq!(terminal.screen_as_string(), "> Hello, World!");

        terminal.advance(CtrlL).unwrap();
        assert_eq!(terminal.get_cursor(), Cursor::new(0, 2));
        assert_eq!(terminal.screen_as_string(), "> ");
    }

    #[test]
    fn scroll() {
        let prompt = "> ";
        let mut terminal = get_terminal(prompt, 4, 10, Cursor::new(0, 0));

        terminal.advance("aaaaaaaa").unwrap();
        terminal.advance("bbbbbbbbbb").unwrap();
        terminal.advance("cccccccccc").unwrap();
        terminal.advance("ddddddddd").unwrap();

        assert_eq!(terminal.get_cursor(), Cursor::new(3, 9));

        assert_eq!(
            terminal.screen_as_string(),
            "> aaaaaaaa\nbbbbbbbbbb\ncccccccccc\nddddddddd"
        );

        terminal.advance("d").unwrap();

        assert_eq!(terminal.get_cursor(), Cursor::new(3, 0));

        assert_eq!(
            terminal.screen_as_string(),
            "bbbbbbbbbb\ncccccccccc\ndddddddddd"
        );

        terminal.advance("eeeeeeeeee").unwrap();

        assert_eq!(
            terminal.screen_as_string(),
            "cccccccccc\ndddddddddd\neeeeeeeeee"
        );

        // terminal.advance(CtrlA);

        // assert_eq!(terminal.get_cursor(), Cursor::new(0, 2));
        // assert_eq!(
        //     terminal.screen_as_string(),
        //     "> aaaaaaaa\nbbbbbbbbbb\ncccccccccc\ndddddddddd"
        // );

        // terminal.advance(CtrlE);
        // assert_eq!(terminal.get_cursor(), Cursor::new(3, 0));
        // assert_eq!(
        //     terminal.screen_as_string(),
        //     "cccccccccc\ndddddddddd\neeeeeeeeee"
        // );
    }

    #[test]
    fn swap() {
        let prompt = "> ";
        let mut terminal = get_terminal(prompt, 4, 10, Cursor::new(0, 0));

        terminal.advance("æøå").unwrap();
        assert_eq!(terminal.screen_as_string(), "> æøå");
        assert_eq!(terminal.get_cursor(), Cursor::new(0, 5));

        assert!(terminal.advance(CtrlT).is_err());

        assert_eq!(terminal.screen_as_string(), "> æøå");
        assert_eq!(terminal.get_cursor(), Cursor::new(0, 5));

        terminal.advance(LEFT).unwrap();

        assert_eq!(terminal.get_cursor(), Cursor::new(0, 4));

        terminal.advance(CtrlT).unwrap();

        assert_eq!(terminal.buffer_as_str(), "æåø");
        assert_eq!(terminal.screen_as_string(), "> æåø");
        assert_eq!(terminal.get_cursor(), Cursor::new(0, 4));

        terminal.advance(CtrlA).unwrap();

        assert_eq!(terminal.get_cursor(), Cursor::new(0, 2));

        assert!(terminal.advance(CtrlT).is_err());
        assert_eq!(terminal.screen_as_string(), "> æåø");
    }

    #[test]
    fn erase_after_cursor() {
        let prompt = "> ";
        let mut terminal = get_terminal(prompt, 4, 10, Cursor::new(0, 0));

        terminal.advance("rm -rf /").unwrap();
        assert_eq!(terminal.get_cursor(), Cursor::new(1, 0));
        assert_eq!(terminal.screen_as_string(), "> rm -rf /");

        terminal.advance(CtrlA).unwrap();
        terminal.advance([CtrlF; 3]).unwrap();

        assert_eq!(terminal.get_cursor(), Cursor::new(0, 5));

        terminal.advance(CtrlK).unwrap();
        assert_eq!(terminal.buffer_as_str(), "rm ");
        assert_eq!(terminal.get_cursor(), Cursor::new(0, 5));
        assert_eq!(terminal.screen_as_string(), "> rm ");
    }

    #[test]
    fn delete_previous_word() {
        let prompt = "> ";
        let mut terminal = get_terminal(prompt, 1, 40, Cursor::new(0, 0));

        terminal.advance("rm file1 file2 file3").unwrap();
        assert_eq!(terminal.screen_as_string(), "> rm file1 file2 file3");

        terminal.advance([CtrlB; 5]).unwrap();

        terminal.advance(CtrlW).unwrap();
        assert_eq!(terminal.get_cursor(), Cursor::new(0, 11));
        assert_eq!(terminal.buffer_as_str(), "rm file1 file3");
        assert_eq!(terminal.screen_as_string(), "> rm file1 file3");

        terminal.advance(CtrlW).unwrap();
        assert_eq!(terminal.screen_as_string(), "> rm file3");
        assert_eq!(terminal.get_cursor(), Cursor::new(0, 5));
    }

    #[test]
    fn delete() {
        let prompt = "> ";
        let mut terminal = get_terminal(prompt, 1, 40, Cursor::new(0, 0));

        assert_eq!(terminal.get_cursor(), Cursor::new(0, 2));
        assert_eq!(terminal.screen_as_string(), "> ");
        terminal.advance("abcde").unwrap();

        terminal.advance(CtrlD).unwrap_err();

        terminal.advance(CtrlA).unwrap();

        terminal.advance(CtrlD).unwrap();
        assert_eq!(terminal.buffer_as_str(), "bcde");
        assert_eq!(terminal.screen_as_string(), "> bcde");

        terminal.advance([RIGHT; 3]).unwrap();
        terminal.advance(CtrlD).unwrap();
        assert_eq!(terminal.buffer_as_str(), "bcd");
        assert_eq!(terminal.screen_as_string(), "> bcd");

        terminal.advance(CtrlD).unwrap_err();

        terminal.advance(CtrlA).unwrap();

        terminal.advance(DELETE).unwrap();
        assert_eq!(terminal.buffer_as_str(), "cd");
        assert_eq!(terminal.screen_as_string(), "> cd");

        terminal.advance(DELETE).unwrap();
        assert_eq!(terminal.buffer_as_str(), "d");
        assert_eq!(terminal.screen_as_string(), "> d");
    }

    #[test]
    fn backspace() {
        let prompt = "> ";
        let mut terminal = get_terminal(prompt, 1, 40, Cursor::new(0, 0));

        assert!(terminal.advance(Backspace).is_err());

        assert_eq!(terminal.get_cursor(), Cursor::new(0, 2));
        assert_eq!(terminal.screen_as_string(), "> ");
        terminal.advance("hello").unwrap();

        terminal.advance(Backspace).unwrap();
        assert_eq!(terminal.buffer_as_str(), "hell");
        assert_eq!(terminal.screen_as_string(), "> hell");

        terminal.advance([LEFT; 2]).unwrap();
        terminal.advance(Backspace).unwrap();
        assert_eq!(terminal.buffer_as_str(), "hll");
        assert_eq!(terminal.screen_as_string(), "> hll");

        terminal.advance(CtrlA).unwrap();
        terminal.advance(Backspace).unwrap_err();
    }
}
