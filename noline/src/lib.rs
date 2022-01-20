#![no_std]

#[cfg(any(test, feature = "std"))]
#[macro_use]
extern crate std;

mod input;
pub mod line_buffer;
mod output;
pub(crate) mod terminal;
mod utf8;

use crate::input::{Action, ControlCharacter::*, Parser, CSI};
use crate::line_buffer::LineBuffer;
use crate::output::CursorMove;

use line_buffer::Buffer;

use output::{Output, OutputAction};
use terminal::Terminal;

use OutputAction::*;
use Status::*;

pub enum Status<'a, B: Buffer> {
    Skip,
    Continue(Output<'a, B>),
    Done(Output<'a, B>),
    Abort(Output<'a, B>),
}

impl<'a, B: Buffer> Status<'a, B> {
    fn iter_bytes(&mut self) -> Option<&mut Output<'a, B>> {
        match self {
            Continue(iter) | Done(iter) | Abort(iter) => Some(iter),
            Skip => None,
        }
    }

    fn is_done(&self) -> Option<Result<(), ()>> {
        match self {
            Skip => None,
            Continue(_) => None,
            Done(_) => Some(Ok(())),
            Abort(_) => Some(Err(())),
        }
    }
}

pub struct Noline<'a, B: Buffer> {
    buffer: &'a mut LineBuffer<B>,
    terminal: Terminal,
    parser: Parser,
    prompt: &'a str,
}

impl<'a, B: Buffer> Noline<'a, B> {
    pub fn new(line_buffer: &'a mut LineBuffer<B>, prompt: &'a str, terminal: Terminal) -> Self {
        Self {
            buffer: line_buffer,
            terminal,
            parser: Parser::new(),
            prompt,
        }
    }

    fn status_continue<'b>(&'b mut self, action: OutputAction) -> Status<'b, B> {
        Status::Continue(Output::new(
            self.prompt,
            &*self.buffer,
            &mut self.terminal,
            action,
        ))
    }

    fn status_done<'b>(&'b mut self, action: OutputAction) -> Status<'b, B> {
        Status::Done(Output::new(
            self.prompt,
            &*self.buffer,
            &mut self.terminal,
            action,
        ))
    }

    fn status_abort<'b>(&'b mut self, action: OutputAction) -> Status<'b, B> {
        Status::Abort(Output::new(
            self.prompt,
            &*self.buffer,
            &mut self.terminal,
            action,
        ))
    }

    pub fn print_prompt<'b>(&'b mut self) -> Status<'b, B> {
        self.status_continue(PrintPrompt)
    }

    fn current_position(&self) -> usize {
        let pos = self.terminal.current_offset() as usize;
        pos - self.prompt.len()
    }

    pub fn advance<'b>(&'b mut self, byte: u8) -> Status<'b, B> {
        let action = self.parser.advance(byte);

        #[cfg(test)]
        dbg!(action);

        match action {
            Action::Print(c) => {
                let pos = self.current_position();

                if self.buffer.insert_utf8_char(pos, c).is_ok() {
                    self.status_continue(PrintBufferAndMoveCursorForward)
                } else {
                    self.status_continue(RingBell)
                }
            }
            Action::ControlCharacter(c) => match c {
                CtrlA => self.status_continue(MoveCursor(CursorMove::Start)),
                CtrlB => self.status_continue(MoveCursor(CursorMove::Back)),
                CtrlC => self.status_abort(PrintNewline),
                CtrlD => {
                    let len = self.buffer.len();

                    if len > 0 {
                        let pos = self.current_position();

                        if pos < len {
                            self.buffer.delete(pos);

                            self.status_continue(EraseAndPrintBuffer)
                        } else {
                            self.status_continue(RingBell)
                        }
                    } else {
                        self.status_abort(PrintNewline)
                    }
                }
                CtrlE => self.status_continue(MoveCursor(CursorMove::End)),
                CtrlF => self.status_continue(MoveCursor(CursorMove::Forward)),
                CtrlK => {
                    let pos = self.current_position();

                    self.buffer.delete_after_char(pos);

                    self.status_continue(EraseAfterCursor)
                }
                CtrlL => {
                    self.buffer.delete_after_char(0);
                    self.status_continue(ClearScreen)
                }
                CtrlT => {
                    let pos = self.current_position();

                    if pos > 0 && pos < self.buffer.as_str().chars().count() {
                        self.buffer.swap_chars(pos);
                        self.status_continue(MoveCursorBackAndPrintBufferAndMoveForward)
                    } else {
                        self.status_continue(RingBell)
                    }
                }
                CtrlU => {
                    self.buffer.delete_after_char(0);
                    self.status_continue(ClearLine)
                }
                CtrlW => {
                    let pos = self.current_position();
                    let move_cursor = -(self.buffer.delete_previous_word(pos) as isize);
                    self.status_continue(MoveCursorAndEraseAndPrintBuffer(move_cursor))
                }
                Enter => self.status_done(PrintNewline),
                CtrlH | Backspace => {
                    let pos = self.current_position();
                    if pos > 0 {
                        self.buffer.delete(pos - 1);
                        self.status_continue(MoveCursorAndEraseAndPrintBuffer(-1))
                    } else {
                        self.status_continue(RingBell)
                    }
                }
                _ => self.status_continue(RingBell),
            },
            Action::ControlSequenceIntroducer(csi) => match csi {
                CSI::CUF(_) => self.status_continue(MoveCursor(CursorMove::Forward)),
                CSI::CUB(_) => self.status_continue(MoveCursor(CursorMove::Back)),
                CSI::Home => self.status_continue(MoveCursor(CursorMove::Start)),
                CSI::Delete => {
                    let len = self.buffer.len();
                    let pos = self.current_position();

                    if pos < len {
                        self.buffer.delete(pos);

                        self.status_continue(EraseAndPrintBuffer)
                    } else {
                        self.status_continue(RingBell)
                    }
                }
                CSI::End => self.status_continue(MoveCursor(CursorMove::End)),
                CSI::Unknown(_) => self.status_continue(RingBell),
                CSI::CUU(_) => self.status_continue(RingBell),
                CSI::CUD(_) => self.status_continue(RingBell),
                CSI::CPR(_, _) => self.status_continue(RingBell),
                CSI::CUP(_, _) => self.status_continue(RingBell),
                CSI::ED(_) => self.status_continue(RingBell),
                CSI::DSR => self.status_continue(RingBell),
                CSI::SU(_) => self.status_continue(RingBell),
                CSI::SD(_) => self.status_continue(RingBell),
            },
            Action::EscapeSequence(_) => self.status_continue(RingBell),
            Action::Ignore => Status::Skip,
            Action::InvalidUtf8 => self.status_continue(RingBell),
        }
    }
}

#[cfg(any(test, feature = "std"))]
pub mod sync {
    use super::*;
    use crate::line_buffer::LineBuffer;
    use crate::terminal::TerminalInitializer;
    use std::io::Read;
    use std::io::Write;

    pub fn readline<'a, B: Buffer, W: Write, R: Read>(
        buffer: &'a mut LineBuffer<B>,
        prompt: &'a str,
        stdin: &mut R,
        stdout: &mut W,
    ) -> Result<&'a str, ()> {
        let mut init = TerminalInitializer::new();

        stdout.write_all(init.init()).unwrap();
        stdout.flush().unwrap();

        let terminal = stdin
            .bytes()
            .map_while(|b| b.ok())
            .find_map(|b| init.advance(b).unwrap())
            .unwrap();

        let mut noline = Noline::new(buffer, prompt, terminal);

        for bytes in noline.print_prompt().iter_bytes().unwrap() {
            stdout.write_all(bytes.as_bytes()).unwrap();
        }

        stdout.flush().unwrap();

        for i in stdin.bytes() {
            if let Ok(byte) = i {
                let mut status = noline.advance(byte);

                if let Some(bytes) = status.iter_bytes() {
                    for b in bytes {
                        stdout.write(b.as_bytes()).unwrap();
                    }
                }

                if let Some(rc) = status.is_done() {
                    drop(status);

                    if rc.is_ok() {
                        return Ok(noline.buffer.as_str());
                    } else {
                        return Err(());
                    }
                }
            } else {
                return Err(());
            }

            stdout.flush().unwrap();
        }
        unreachable!();
    }
}

#[cfg(test)]
mod tests {
    use std::vec::Vec;

    use std::string::String;

    use std::boxed::Box;

    use crate::input::tests::AsByteVec;
    use crate::line_buffer::AllocLineBuffer;
    use crate::terminal::{Cursor, TerminalInitializer};

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
            let mut init = TerminalInitializer::new();

            let size = format!(
                "\x1b[{};{}R\x1b[{};{}R",
                origin.row + 1,
                origin.column + 1,
                rows,
                columns
            );

            let terminal = size
                .as_bytes()
                .iter()
                .find_map(|&b| init.advance(b).unwrap())
                .unwrap();

            Self {
                noline: Some(Noline::new(buffer, prompt, terminal)),
                parser: Parser::new(),
                screen: vec![vec!['\0'; columns]; rows],
                cursor: origin,
                rows,
                columns,
                saved_cursor: None,
                bell: false,
            }
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

        fn handle_input(&mut self, mock_term_action: Action) {
            dbg!(mock_term_action);
            match mock_term_action {
                Action::Ignore => (),
                Action::Print(c) => {
                    let pos = self.cursor.column;
                    let line = self.current_line();

                    line[pos] = c.to_char();
                    self.move_column(1);
                }
                Action::ControlSequenceIntroducer(csi) => match csi {
                    CSI::CUU(_) => unimplemented!(),
                    CSI::CUD(_) => unimplemented!(),
                    CSI::CUF(_) => unimplemented!(),
                    CSI::CUB(_) => unimplemented!(),
                    CSI::CPR(_, _) => unimplemented!(),
                    CSI::CUP(row, column) => {
                        self.cursor = Cursor::new(row - 1, column - 1);
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
                    CSI::DSR => unimplemented!(),
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
                },
                Action::InvalidUtf8 => unreachable!(),
                Action::ControlCharacter(ctrl) => {
                    dbg!(ctrl);

                    match ctrl {
                        CtrlJ => self.cursor.column = 0,
                        Enter => {
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
        }

        fn print_prompt(&mut self) {
            let mut noline = self.noline.take().unwrap();
            let mut prompt = noline.print_prompt();

            if let Some(output) = prompt.iter_bytes() {
                for bytes in output {
                    for byte in bytes.as_bytes() {
                        let action = self.parser.advance(*byte);

                        self.handle_input(action);
                    }
                }
            }

            drop(prompt);

            self.noline = Some(noline);
        }

        fn advance(&mut self, input: impl AsByteVec) -> Result<(), ()> {
            self.bell = false;
            let mut noline = self.noline.take().unwrap();

            for byte in input.as_byte_vec() {
                let mut status = noline.advance(byte);

                if let Some(output) = status.iter_bytes() {
                    for bytes in output {
                        for byte in bytes.as_bytes() {
                            let action = self.parser.advance(*byte);

                            self.handle_input(action);
                        }
                    }
                }
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
        let mut terminal = MockTerminal::new(buffer, prompt, rows, columns, origin);

        assert_eq!(terminal.get_cursor(), Cursor::new(origin.row, 0));

        terminal.print_prompt();
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
