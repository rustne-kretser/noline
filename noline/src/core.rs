//! Core library containing the components for building an editor.
//!
//! Use [`Initializer`] to get [`crate::terminal::Terminal`] and then
//! use [`Line`] to read a single line.

use crate::history::{History, HistoryNavigator};
use crate::input::{Action, ControlCharacter::*, Parser, CSI};
use crate::line_buffer::Buffer;
use crate::line_buffer::LineBuffer;
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
pub(crate) enum InitializerState {
    New,
    Position(Cursor),
    Done,
}

// Terminal initializer used to probe terminal size. To use, print
// bytes return [`Initializer::init()'] to output and read bytes into
// [`initializer::advance()`] until it ruturns item.
pub struct Initializer {
    pub(crate) state: InitializerState,
    parser: Parser,
}

impl Initializer {
    pub fn new() -> Self {
        Self {
            state: InitializerState::New,
            parser: Parser::new(),
        }
    }

    // Returns initialization bytes.
    pub fn init() -> &'static [u8] {
        // There is no command to request the size of the terminal window,
        // so to probe the size we move the cursor way out of the screen
        // and then request the position, because the cursor must be in
        // the screen this gives us the size.
        "\r\x1b[J\x1b7\x1b[6n\x1b[999;999H\x1b[6n\x1b8".as_bytes()
    }

    // Advance initializer by one byte
    pub fn advance(&mut self, byte: u8) -> InitializerResult<Terminal> {
        let action = self.parser.advance(byte);

        #[cfg(test)]
        dbg!(byte, action, &self.state);

        match action {
            Action::ControlSequenceIntroducer(CSI::CPR(x, y)) => match self.state {
                InitializerState::New => {
                    self.state = InitializerState::Position(Cursor::new(x - 1, y - 1));
                    InitializerResult::Continue
                }
                InitializerState::Position(pos) => {
                    #[cfg(test)]
                    dbg!(pos, x, y);

                    self.state = InitializerState::Done;
                    InitializerResult::Item(Terminal::new(x, y, pos))
                }
                InitializerState::Done => InitializerResult::InvalidInput,
            },
            Action::Ignore => InitializerResult::Continue,
            _ => InitializerResult::InvalidInput,
        }
    }
}

// State machine for reading single line.
//
// Provide input by calling [`Line::advance`], returning
// [`crate::output::Output`] object, which
//
// Before reading line, call [`Line::reset`] to truncate buffer, clear
// line, get cursor position and print prompt. Call [`Line::advance`]
// for each byte read from input and print bytes from
// [`crate::output::Output`] to output.
pub struct Line<'a, B: Buffer, H: History> {
    buffer: &'a mut LineBuffer<B>,
    terminal: &'a mut Terminal,
    parser: Parser,
    prompt: &'a str,
    nav: HistoryNavigator<'a, H>,
}

impl<'a, B: Buffer, H: History> Line<'a, B, H> {
    pub fn new(
        prompt: &'a str,
        buffer: &'a mut LineBuffer<B>,
        terminal: &'a mut Terminal,
        history: &'a mut H,
    ) -> Self {
        Self {
            buffer,
            terminal,
            parser: Parser::new(),
            prompt,
            nav: HistoryNavigator::new(history),
        }
    }

    // Truncate buffer, clear line and print prompt
    pub fn reset<'b>(&'b mut self) -> Output<'b, B> {
        self.buffer.truncate();
        self.generate_output(ClearAndPrintPrompt)
    }

    fn generate_output<'b>(&'b mut self, action: OutputAction) -> Output<'b, B> {
        Output::new(self.prompt, &mut self.buffer, &mut self.terminal, action)
    }

    fn current_position(&self) -> usize {
        let pos = self.terminal.current_offset() as usize;
        pos - self.prompt.len()
    }

    fn history_move_up<'b>(&'b mut self) -> Output<'b, B> {
        let entry = if self.nav.is_active() {
            self.nav.move_up()
        } else if self.buffer.len() == 0 {
            self.nav.reset();
            self.nav.move_up()
        } else {
            Err(())
        };

        if let Ok(entry) = entry {
            let (slice1, slice2) = entry.get_slices();

            self.buffer.truncate();
            unsafe {
                self.buffer.insert_bytes(0, slice1).unwrap();
                self.buffer.insert_bytes(slice1.len(), slice2).unwrap();
            }

            self.generate_output(ClearAndPrintBuffer)
        } else {
            self.generate_output(RingBell)
        }
    }

    fn history_move_down<'b>(&'b mut self) -> Output<'b, B> {
        let entry = if self.nav.is_active() {
            self.nav.move_down()
        } else {
            return self.generate_output(RingBell);
        };

        if let Ok(entry) = entry {
            let (slice1, slice2) = entry.get_slices();

            self.buffer.truncate();
            unsafe {
                self.buffer.insert_bytes(0, slice1).unwrap();
                self.buffer.insert_bytes(slice1.len(), slice2).unwrap();
            }
        } else {
            self.nav.reset();
            self.buffer.truncate();
        }

        self.generate_output(ClearAndPrintBuffer)
    }

    // Advance state machine by one byte. Returns output iterator over
    // 0 or more byte slices.
    pub(crate) fn advance<'b>(&'b mut self, byte: u8) -> Output<'b, B> {
        let action = self.parser.advance(byte);

        #[cfg(test)]
        dbg!(action);

        match action {
            Action::Print(c) => {
                let pos = self.current_position();

                if self.buffer.insert_utf8_char(pos, c).is_ok() {
                    self.generate_output(PrintBufferAndMoveCursorForward)
                } else {
                    self.generate_output(RingBell)
                }
            }
            Action::ControlCharacter(c) => match c {
                CtrlA => self.generate_output(MoveCursor(CursorMove::Start)),
                CtrlB => self.generate_output(MoveCursor(CursorMove::Back)),
                CtrlC => self.generate_output(Abort),
                CtrlD => {
                    let len = self.buffer.len();

                    if len > 0 {
                        let pos = self.current_position();

                        if pos < len {
                            self.buffer.delete(pos);

                            self.generate_output(EraseAndPrintBuffer)
                        } else {
                            self.generate_output(RingBell)
                        }
                    } else {
                        self.generate_output(Abort)
                    }
                }
                CtrlE => self.generate_output(MoveCursor(CursorMove::End)),
                CtrlF => self.generate_output(MoveCursor(CursorMove::Forward)),
                CtrlK => {
                    let pos = self.current_position();

                    self.buffer.delete_after_char(pos);

                    self.generate_output(EraseAfterCursor)
                }
                CtrlL => {
                    self.buffer.delete_after_char(0);
                    self.generate_output(ClearScreen)
                }
                CtrlN => self.history_move_down(),
                CtrlP => self.history_move_up(),
                CtrlT => {
                    let pos = self.current_position();

                    if pos > 0 && pos < self.buffer.as_str().chars().count() {
                        self.buffer.swap_chars(pos);
                        self.generate_output(MoveCursorBackAndPrintBufferAndMoveForward)
                    } else {
                        self.generate_output(RingBell)
                    }
                }
                CtrlU => {
                    self.buffer.delete_after_char(0);
                    self.generate_output(ClearLine)
                }
                CtrlW => {
                    let pos = self.current_position();
                    let move_cursor = -(self.buffer.delete_previous_word(pos) as isize);
                    self.generate_output(MoveCursorAndEraseAndPrintBuffer(move_cursor))
                }
                CarriageReturn | LineFeed => {
                    if self.buffer.len() > 0 {
                        let _ = self.nav.history.add_entry(self.buffer.as_str());
                    }

                    self.generate_output(Done)
                }
                CtrlH | Backspace => {
                    let pos = self.current_position();
                    if pos > 0 {
                        self.buffer.delete(pos - 1);
                        self.generate_output(MoveCursorAndEraseAndPrintBuffer(-1))
                    } else {
                        self.generate_output(RingBell)
                    }
                }
                _ => self.generate_output(RingBell),
            },
            Action::ControlSequenceIntroducer(csi) => match csi {
                CSI::CUF(_) => self.generate_output(MoveCursor(CursorMove::Forward)),
                CSI::CUB(_) => self.generate_output(MoveCursor(CursorMove::Back)),
                CSI::Home => self.generate_output(MoveCursor(CursorMove::Start)),
                CSI::Delete => {
                    let len = self.buffer.len();
                    let pos = self.current_position();

                    if pos < len {
                        self.buffer.delete(pos);

                        self.generate_output(EraseAndPrintBuffer)
                    } else {
                        self.generate_output(RingBell)
                    }
                }
                CSI::End => self.generate_output(MoveCursor(CursorMove::End)),
                CSI::CPR(row, column) => {
                    let cursor = Cursor::new(row - 1, column - 1);
                    self.terminal.reset(cursor);
                    self.generate_output(Nothing)
                }
                CSI::Unknown(_) => self.generate_output(RingBell),
                CSI::CUU(_) => self.history_move_up(),
                CSI::CUD(_) => self.history_move_down(),
                CSI::CUP(_, _) => self.generate_output(RingBell),
                CSI::ED(_) => self.generate_output(RingBell),
                CSI::DSR => self.generate_output(RingBell),
                CSI::SU(_) => self.generate_output(RingBell),
                CSI::SD(_) => self.generate_output(RingBell),
            },
            Action::EscapeSequence(_) => self.generate_output(RingBell),
            Action::Ignore => self.generate_output(Nothing),
            Action::InvalidUtf8 => self.generate_output(RingBell),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::vec::Vec;

    use std::string::String;

    use crate::history::{NoHistory, StaticHistory, UnboundedHistory};
    use crate::line_buffer::StaticBuffer;
    use crate::terminal::Cursor;
    use crate::testlib::{csi, AsByteVec, MockTerminal};

    use super::*;

    struct Editor<B: Buffer, H: History> {
        buffer: LineBuffer<B>,
        terminal: Terminal,
        history: H,
    }

    impl<B: Buffer, H: History> Editor<B, H> {
        fn new(term: &mut MockTerminal) -> Self {
            let mut initializer = Initializer::new();

            let terminal = Initializer::init()
                .iter()
                .map(|&b| term.advance(b))
                .filter_map(|output| output.and_then(|x| Some(x.into_iter())))
                .flatten()
                .collect::<Vec<u8>>()
                .into_iter()
                .find_map(|b| match initializer.advance(b) {
                    InitializerResult::Continue => None,
                    InitializerResult::Item(terminal) => Some(terminal),
                    InitializerResult::InvalidInput => unreachable!(),
                })
                .unwrap();

            Self {
                buffer: LineBuffer::new(),
                terminal,
                history: H::default(),
            }
        }

        fn get_line<'b>(
            &'b mut self,
            prompt: &'b str,
            mockterm: &mut MockTerminal,
        ) -> Line<'b, B, H> {
            let cursor = mockterm.get_cursor();
            let mut line = Line::new(
                prompt,
                &mut self.buffer,
                &mut self.terminal,
                &mut self.history,
            );

            let output: Vec<u8> = line
                .reset()
                .into_iter()
                .filter_map(|item| {
                    item.get_bytes()
                        .and_then(|bytes| Some(bytes.iter().map(|&b| b).collect::<Vec<u8>>()))
                })
                .flatten()
                .filter_map(|b| mockterm.advance(b))
                .flatten()
                .collect();

            output.into_iter().for_each(|b| {
                line.advance(b)
                    .into_iter()
                    .for_each(|output| assert!(output.get_bytes().is_none()))
            });

            assert_eq!(mockterm.current_line_as_string(), prompt);
            assert_eq!(mockterm.get_cursor(), Cursor::new(cursor.row, prompt.len()));

            line
        }
    }

    fn advance<'a, B: Buffer, H: History>(
        terminal: &mut MockTerminal,
        noline: &mut Line<'a, B, H>,
        input: impl AsByteVec,
    ) -> core::result::Result<(), ()> {
        terminal.bell = false;

        for input in input.as_byte_vec() {
            for item in noline.advance(input) {
                if let Some(bytes) = item.get_bytes() {
                    for &b in bytes {
                        terminal.advance(b);
                    }
                }
            }
        }

        assert_eq!(noline.terminal.get_cursor(), terminal.cursor);

        dbg!(terminal.screen_as_string());

        if terminal.bell {
            Err(())
        } else {
            Ok(())
        }
    }

    fn get_terminal_and_editor<'a>(
        rows: usize,
        columns: usize,
        origin: Cursor,
    ) -> (MockTerminal, Editor<Vec<u8>, NoHistory>) {
        let mut terminal = MockTerminal::new(rows, columns, origin);

        let editor = Editor::new(&mut terminal);

        assert_eq!(terminal.get_cursor(), origin);

        (terminal, editor)
    }

    #[test]
    fn movecursor() {
        let prompt = "> ";
        let (mut terminal, mut editor) = get_terminal_and_editor(4, 10, Cursor::new(1, 0));

        let mut line = editor.get_line(prompt, &mut terminal);

        advance(&mut terminal, &mut line, "Hello, World!").unwrap();
        assert_eq!(terminal.get_cursor(), Cursor::new(2, 5));

        advance(&mut terminal, &mut line, [csi::LEFT; 6]).unwrap();
        assert_eq!(terminal.get_cursor(), Cursor::new(1, 9));

        advance(&mut terminal, &mut line, CtrlA).unwrap();

        assert_eq!(terminal.get_cursor(), Cursor::new(1, 2));

        assert!(advance(&mut terminal, &mut line, csi::LEFT).is_err());

        assert_eq!(terminal.get_cursor(), Cursor::new(1, 2));

        advance(&mut terminal, &mut line, CtrlE).unwrap();

        assert_eq!(terminal.get_cursor(), Cursor::new(2, 5));

        assert!(advance(&mut terminal, &mut line, csi::RIGHT).is_err());

        assert_eq!(terminal.get_cursor(), Cursor::new(2, 5));

        advance(&mut terminal, &mut line, csi::HOME).unwrap();

        assert_eq!(terminal.get_cursor(), Cursor::new(1, 2));

        advance(&mut terminal, &mut line, csi::END).unwrap();

        assert_eq!(terminal.get_cursor(), Cursor::new(2, 5));
    }

    #[test]
    fn cursor_scroll() {
        let prompt = "> ";
        let (mut terminal, mut editor) = get_terminal_and_editor(4, 10, Cursor::new(3, 0));

        let mut line = editor.get_line(prompt, &mut terminal);

        advance(&mut terminal, &mut line, "23456789").unwrap();
        assert_eq!(terminal.get_cursor(), Cursor::new(3, 0));
    }

    #[test]
    fn clear_line() {
        let prompt = "> ";
        let (mut terminal, mut editor) = get_terminal_and_editor(4, 20, Cursor::new(1, 0));

        let mut line = editor.get_line(prompt, &mut terminal);

        advance(&mut terminal, &mut line, "Hello, World!").unwrap();
        assert_eq!(terminal.get_cursor(), Cursor::new(1, 15));
        assert_eq!(terminal.screen_as_string(), "> Hello, World!");

        advance(&mut terminal, &mut line, CtrlU).unwrap();
        assert_eq!(terminal.get_cursor(), Cursor::new(1, 2));
        assert_eq!(terminal.screen_as_string(), "> ");
    }

    #[test]
    fn clear_screen() {
        let prompt = "> ";
        let (mut terminal, mut editor) = get_terminal_and_editor(4, 20, Cursor::new(1, 0));

        let mut line = editor.get_line(prompt, &mut terminal);

        advance(&mut terminal, &mut line, "Hello, World!").unwrap();
        assert_eq!(terminal.get_cursor(), Cursor::new(1, 15));
        assert_eq!(terminal.screen_as_string(), "> Hello, World!");

        advance(&mut terminal, &mut line, CtrlL).unwrap();
        assert_eq!(terminal.get_cursor(), Cursor::new(0, 2));
        assert_eq!(terminal.screen_as_string(), "> ");
    }

    #[test]
    fn scroll() {
        let prompt = "> ";
        let (mut terminal, mut editor) = get_terminal_and_editor(4, 10, Cursor::new(0, 0));

        let mut line = editor.get_line(prompt, &mut terminal);

        advance(&mut terminal, &mut line, "aaaaaaaa").unwrap();
        advance(&mut terminal, &mut line, "bbbbbbbbbb").unwrap();
        advance(&mut terminal, &mut line, "cccccccccc").unwrap();
        advance(&mut terminal, &mut line, "ddddddddd").unwrap();

        assert_eq!(terminal.get_cursor(), Cursor::new(3, 9));

        assert_eq!(
            terminal.screen_as_string(),
            "> aaaaaaaa\nbbbbbbbbbb\ncccccccccc\nddddddddd"
        );

        advance(&mut terminal, &mut line, "d").unwrap();

        assert_eq!(terminal.get_cursor(), Cursor::new(3, 0));

        assert_eq!(
            terminal.screen_as_string(),
            "bbbbbbbbbb\ncccccccccc\ndddddddddd"
        );

        advance(&mut terminal, &mut line, "eeeeeeeeee").unwrap();

        assert_eq!(
            terminal.screen_as_string(),
            "cccccccccc\ndddddddddd\neeeeeeeeee"
        );

        // advance(&mut terminal, &mut noline, CtrlA);

        // assert_eq!(terminal.get_cursor(), Cursor::new(0, 2));
        // assert_eq!(
        //     terminal.screen_as_string(),
        //     "> aaaaaaaa\nbbbbbbbbbb\ncccccccccc\ndddddddddd"
        // );

        // advance(&mut terminal, &mut noline, CtrlE);
        // assert_eq!(terminal.get_cursor(), Cursor::new(3, 0));
        // assert_eq!(
        //     terminal.screen_as_string(),
        //     "cccccccccc\ndddddddddd\neeeeeeeeee"
        // );
    }

    #[test]
    fn swap() {
        let prompt = "> ";
        let (mut terminal, mut editor) = get_terminal_and_editor(4, 10, Cursor::new(0, 0));

        let mut line = editor.get_line(prompt, &mut terminal);

        advance(&mut terminal, &mut line, "æøå").unwrap();
        assert_eq!(terminal.screen_as_string(), "> æøå");
        assert_eq!(terminal.get_cursor(), Cursor::new(0, 5));

        assert!(advance(&mut terminal, &mut line, CtrlT).is_err());

        assert_eq!(terminal.screen_as_string(), "> æøå");
        assert_eq!(terminal.get_cursor(), Cursor::new(0, 5));

        advance(&mut terminal, &mut line, csi::LEFT).unwrap();

        assert_eq!(terminal.get_cursor(), Cursor::new(0, 4));

        advance(&mut terminal, &mut line, CtrlT).unwrap();

        assert_eq!(line.buffer.as_str(), "æåø");
        assert_eq!(terminal.screen_as_string(), "> æåø");
        assert_eq!(terminal.get_cursor(), Cursor::new(0, 4));

        advance(&mut terminal, &mut line, CtrlA).unwrap();

        assert_eq!(terminal.get_cursor(), Cursor::new(0, 2));

        assert!(advance(&mut terminal, &mut line, CtrlT).is_err());
        assert_eq!(terminal.screen_as_string(), "> æåø");
    }

    #[test]
    fn erase_after_cursor() {
        let prompt = "> ";
        let (mut terminal, mut editor) = get_terminal_and_editor(4, 10, Cursor::new(0, 0));

        let mut line = editor.get_line(prompt, &mut terminal);

        advance(&mut terminal, &mut line, "rm -rf /").unwrap();
        assert_eq!(terminal.get_cursor(), Cursor::new(1, 0));
        assert_eq!(terminal.screen_as_string(), "> rm -rf /");

        advance(&mut terminal, &mut line, CtrlA).unwrap();
        advance(&mut terminal, &mut line, [CtrlF; 3]).unwrap();

        assert_eq!(terminal.get_cursor(), Cursor::new(0, 5));

        advance(&mut terminal, &mut line, CtrlK).unwrap();
        assert_eq!(line.buffer.as_str(), "rm ");
        assert_eq!(terminal.get_cursor(), Cursor::new(0, 5));
        assert_eq!(terminal.screen_as_string(), "> rm ");
    }

    #[test]
    fn delete_previous_word() {
        let prompt = "> ";
        let (mut terminal, mut editor) = get_terminal_and_editor(1, 40, Cursor::new(0, 0));

        let mut line = editor.get_line(prompt, &mut terminal);

        advance(&mut terminal, &mut line, "rm file1 file2 file3").unwrap();
        assert_eq!(terminal.screen_as_string(), "> rm file1 file2 file3");

        advance(&mut terminal, &mut line, [CtrlB; 5]).unwrap();

        advance(&mut terminal, &mut line, CtrlW).unwrap();
        assert_eq!(terminal.get_cursor(), Cursor::new(0, 11));
        assert_eq!(line.buffer.as_str(), "rm file1 file3");
        assert_eq!(terminal.screen_as_string(), "> rm file1 file3");

        advance(&mut terminal, &mut line, CtrlW).unwrap();
        assert_eq!(terminal.screen_as_string(), "> rm file3");
        assert_eq!(terminal.get_cursor(), Cursor::new(0, 5));
    }

    #[test]
    fn delete() {
        let prompt = "> ";
        let (mut terminal, mut editor) = get_terminal_and_editor(1, 40, Cursor::new(0, 0));

        let mut line = editor.get_line(prompt, &mut terminal);

        assert_eq!(terminal.get_cursor(), Cursor::new(0, 2));
        assert_eq!(terminal.screen_as_string(), "> ");
        advance(&mut terminal, &mut line, "abcde").unwrap();

        advance(&mut terminal, &mut line, CtrlD).unwrap_err();

        advance(&mut terminal, &mut line, CtrlA).unwrap();

        advance(&mut terminal, &mut line, CtrlD).unwrap();
        assert_eq!(line.buffer.as_str(), "bcde");
        assert_eq!(terminal.screen_as_string(), "> bcde");

        advance(&mut terminal, &mut line, [csi::RIGHT; 3]).unwrap();
        advance(&mut terminal, &mut line, CtrlD).unwrap();
        assert_eq!(line.buffer.as_str(), "bcd");
        assert_eq!(terminal.screen_as_string(), "> bcd");

        advance(&mut terminal, &mut line, CtrlD).unwrap_err();

        advance(&mut terminal, &mut line, CtrlA).unwrap();

        advance(&mut terminal, &mut line, csi::DELETE).unwrap();
        assert_eq!(line.buffer.as_str(), "cd");
        assert_eq!(terminal.screen_as_string(), "> cd");

        advance(&mut terminal, &mut line, csi::DELETE).unwrap();
        assert_eq!(line.buffer.as_str(), "d");
        assert_eq!(terminal.screen_as_string(), "> d");
    }

    #[test]
    fn backspace() {
        let prompt = "> ";
        let (mut terminal, mut editor) = get_terminal_and_editor(1, 40, Cursor::new(0, 0));

        let mut line = editor.get_line(prompt, &mut terminal);

        assert!(advance(&mut terminal, &mut line, Backspace).is_err());

        assert_eq!(terminal.get_cursor(), Cursor::new(0, 2));
        assert_eq!(terminal.screen_as_string(), "> ");
        advance(&mut terminal, &mut line, "hello").unwrap();

        advance(&mut terminal, &mut line, Backspace).unwrap();
        assert_eq!(line.buffer.as_str(), "hell");
        assert_eq!(terminal.screen_as_string(), "> hell");

        advance(&mut terminal, &mut line, [csi::LEFT; 2]).unwrap();
        advance(&mut terminal, &mut line, Backspace).unwrap();
        assert_eq!(line.buffer.as_str(), "hll");
        assert_eq!(terminal.screen_as_string(), "> hll");

        advance(&mut terminal, &mut line, CtrlA).unwrap();
        advance(&mut terminal, &mut line, Backspace).unwrap_err();
    }

    #[test]
    fn static_buffer() {
        let mut terminal = MockTerminal::new(20, 80, Cursor::new(0, 0));
        let mut editor: Editor<StaticBuffer<20>, NoHistory> = Editor::new(&mut terminal);

        let mut line = editor.get_line("> ", &mut terminal);

        let input: String = (0..20).map(|_| "a").collect();

        advance(&mut terminal, &mut line, input.as_str()).unwrap();

        assert_eq!(advance(&mut terminal, &mut line, "a"), Err(()));

        assert_eq!(line.buffer.as_str(), input);

        advance(&mut terminal, &mut line, Backspace).unwrap();
    }

    #[test]
    fn history() {
        fn test<H: History>() {
            let mut terminal = MockTerminal::new(20, 80, Cursor::new(0, 0));
            let mut editor: Editor<StaticBuffer<20>, H> = Editor::new(&mut terminal);

            let mut line = editor.get_line("> ", &mut terminal);

            advance(&mut terminal, &mut line, "this is a line\r").unwrap();

            let mut line = editor.get_line("> ", &mut terminal);

            assert_eq!(terminal.screen_as_string(), "> this is a line\n> ");

            assert!(advance(&mut terminal, &mut line, csi::UP).is_ok());

            assert_eq!(
                terminal.screen_as_string(),
                "> this is a line\n> this is a line"
            );

            assert!(advance(&mut terminal, &mut line, csi::DOWN).is_ok());

            assert_eq!(terminal.screen_as_string(), "> this is a line\n> ");

            advance(&mut terminal, &mut line, "another line\r").unwrap();

            let mut line = editor.get_line("> ", &mut terminal);
            advance(&mut terminal, &mut line, "yet another line\r").unwrap();

            let mut line = editor.get_line("> ", &mut terminal);

            assert_eq!(
                terminal.screen_as_string(),
                "> this is a line\n> another line\n> yet another line\n> "
            );

            assert!(advance(&mut terminal, &mut line, csi::UP).is_ok());

            assert_eq!(
                terminal.screen_as_string(),
                "> this is a line\n> another line\n> yet another line\n> yet another line"
            );

            assert!(advance(&mut terminal, &mut line, csi::UP).is_ok());

            assert_eq!(
                terminal.screen_as_string(),
                "> this is a line\n> another line\n> yet another line\n> another line"
            );

            assert!(advance(&mut terminal, &mut line, csi::UP).is_ok());

            assert_eq!(
                terminal.screen_as_string(),
                "> this is a line\n> another line\n> yet another line\n> this is a line"
            );

            assert!(advance(&mut terminal, &mut line, csi::UP).is_err());

            assert_eq!(
                terminal.screen_as_string(),
                "> this is a line\n> another line\n> yet another line\n> this is a line"
            );

            assert!(advance(&mut terminal, &mut line, csi::DOWN).is_ok());

            assert_eq!(
                terminal.screen_as_string(),
                "> this is a line\n> another line\n> yet another line\n> another line"
            );

            assert!(advance(&mut terminal, &mut line, csi::DOWN).is_ok());

            assert_eq!(
                terminal.screen_as_string(),
                "> this is a line\n> another line\n> yet another line\n> yet another line"
            );
            assert!(advance(&mut terminal, &mut line, csi::DOWN).is_ok());

            assert_eq!(
                terminal.screen_as_string(),
                "> this is a line\n> another line\n> yet another line\n> "
            );

            assert!(advance(&mut terminal, &mut line, csi::DOWN).is_err());

            assert_eq!(
                terminal.screen_as_string(),
                "> this is a line\n> another line\n> yet another line\n> "
            );
        }

        test::<UnboundedHistory>();
        test::<StaticHistory<128>>();
    }
}
