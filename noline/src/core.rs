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

enum ResetState {
    New,
    GetSize,
    GetPosition,
    Done,
}

pub struct ResetHandle<'line, 'a, B: Buffer, H: History, I> {
    line: &'line mut Line<'a, B, H, I>,
    state: ResetState,
}

impl<'line, 'a, 'item, 'output, B, H, I> ResetHandle<'line, 'a, B, H, I>
where
    I: Iterator<Item = &'item str> + Clone + 'a,
    B: Buffer,
    H: History,
    'item: 'output,
{
    fn new(line: &'line mut Line<'a, B, H, I>) -> Self {
        Self {
            line,
            state: ResetState::New,
        }
    }

    pub fn start(&mut self) -> Output<'_, B, I> {
        assert!(matches!(self.state, ResetState::New));
        self.state = ResetState::GetSize;

        self.line.generate_output(ProbeSize)
    }

    pub fn advance(&mut self, byte: u8) -> Option<Output<'_, B, I>> {
        let action = self.line.parser.advance(byte);

        match action {
            Action::ControlSequenceIntroducer(CSI::CPR(x, y)) => match self.state {
                ResetState::New => panic!("Invalid state"),
                ResetState::GetSize => {
                    self.line.terminal.resize(x, y);
                    self.state = ResetState::GetPosition;
                    Some(self.line.generate_output(ClearAndPrintPrompt))
                }
                ResetState::GetPosition => {
                    #[cfg(test)]
                    dbg!(x, y);
                    self.line.terminal.reset(Cursor::new(x - 1, y - 1));
                    self.state = ResetState::Done;
                    None
                }
                ResetState::Done => panic!("Invalid state"),
            },
            Action::Ignore => Some(self.line.generate_output(Nothing)),
            _ => None,
        }
    }
}

#[cfg_attr(test, derive(Debug))]
pub struct Prompt<I> {
    parts: I,
    len: usize,
}

impl<'a, I> Prompt<I>
where
    I: Iterator<Item = &'a str> + Clone,
{
    fn new(parts: I) -> Self {
        Self {
            len: parts.clone().map(|part| part.len()).sum(),
            parts,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }
}

impl<'a, I> Prompt<I>
where
    I: Iterator<Item = &'a str> + Clone,
{
    pub fn iter(&self) -> I {
        self.parts.clone()
    }
}

#[derive(Clone)]
pub struct StrIter<'a> {
    s: Option<&'a str>,
}

impl<'a> Iterator for StrIter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        self.s.take()
    }
}

impl<'a> From<&'a str> for Prompt<StrIter<'a>> {
    fn from(value: &'a str) -> Self {
        Self::new(StrIter { s: Some(value) })
    }
}

impl<'a, I> From<I> for Prompt<I>
where
    I: Iterator<Item = &'a str> + Clone,
{
    fn from(value: I) -> Self {
        Self::new(value)
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
pub struct Line<'a, B: Buffer, H: History, I> {
    buffer: &'a mut LineBuffer<B>,
    terminal: &'a mut Terminal,
    parser: Parser,
    prompt: Prompt<I>,
    nav: HistoryNavigator<'a, H>,
}

impl<'a, 'item, 'output, B: Buffer, H: History, I> Line<'a, B, H, I>
where
    I: Iterator<Item = &'item str> + Clone + 'a,
    'item: 'output,
    'a: 'output,
{
    pub fn new(
        prompt: impl Into<Prompt<I>>,
        buffer: &'a mut LineBuffer<B>,
        terminal: &'a mut Terminal,
        history: &'a mut H,
    ) -> Self {
        Self {
            buffer,
            terminal,
            parser: Parser::new(),
            prompt: prompt.into(),
            nav: HistoryNavigator::new(history),
        }
    }

    // Truncate buffer, clear line and print prompt
    pub fn reset(&mut self) -> ResetHandle<'_, 'a, B, H, I> {
        self.buffer.truncate();
        ResetHandle::new(self)
    }

    fn generate_output(&mut self, action: OutputAction) -> Output<'_, B, I> {
        Output::new(&self.prompt, self.buffer, self.terminal, action)
    }

    fn current_position(&self) -> usize {
        let pos = self.terminal.current_offset() as usize;
        pos - self.prompt.len()
    }

    fn history_move_up(&mut self) -> Output<'_, B, I> {
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

    fn history_move_down(&mut self) -> Output<'_, B, I> {
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
    pub(crate) fn advance(&mut self, byte: u8) -> Output<'_, B, I> {
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

    use crate::history::{NoHistory, SliceHistory, UnboundedHistory};
    use crate::line_buffer::UnboundedBuffer;
    use crate::terminal::Cursor;
    use crate::testlib::{csi, MockTerminal, ToByteVec};

    use super::*;

    struct Editor<B: Buffer, H: History> {
        buffer: LineBuffer<B>,
        terminal: Terminal,
        history: H,
    }

    impl<B: Buffer, H: History> Editor<B, H> {
        fn new(buffer: LineBuffer<B>, history: H) -> Self {
            let terminal = Terminal::default();

            Self {
                buffer,
                terminal,
                history,
            }
        }

        fn get_line(
            &mut self,
            prompt: &'static str,
            mockterm: &mut MockTerminal,
        ) -> Line<'_, B, H, StrIter> {
            let cursor = mockterm.get_cursor();
            let mut line = Line::new(
                prompt,
                &mut self.buffer,
                &mut self.terminal,
                &mut self.history,
            );

            let mut reset = line.reset();

            let mut reset_start: Vec<u8> = reset
                .start()
                .into_iter()
                .filter_map(|item| item.get_bytes().map(|bytes| bytes.to_vec()))
                .flatten()
                .collect();

            while !reset_start.is_empty() {
                let term_response: Vec<u8> = reset_start
                    .into_iter()
                    .filter_map(|b| mockterm.advance(b))
                    .flat_map(|output| output.into_iter())
                    .collect();

                reset_start = term_response
                    .iter()
                    .copied()
                    .filter_map(|b| {
                        reset.advance(b).map(|output| {
                            output
                                .into_iter()
                                .map(|item| item.get_bytes().map(|bytes| bytes.to_vec()))
                                .collect::<Vec<_>>()
                        })
                    })
                    .flatten()
                    .flatten()
                    .flatten()
                    .collect();
            }

            assert_eq!(mockterm.current_line_as_string(), prompt);
            assert_eq!(mockterm.get_cursor(), Cursor::new(cursor.row, prompt.len()));

            line
        }
    }

    fn advance<'a, B: Buffer, H: History>(
        terminal: &mut MockTerminal,
        noline: &mut Line<'a, B, H, StrIter<'a>>,
        input: impl ToByteVec,
    ) -> core::result::Result<(), ()> {
        terminal.bell = false;

        for input in input.to_byte_vec() {
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

    fn get_terminal_and_editor(
        rows: usize,
        columns: usize,
        origin: Cursor,
    ) -> (MockTerminal, Editor<UnboundedBuffer, NoHistory>) {
        let terminal = MockTerminal::new(rows, columns, origin);

        let editor = Editor::new(LineBuffer::new_unbounded(), NoHistory {});

        assert_eq!(terminal.get_cursor(), origin);

        (terminal, editor)
    }

    #[test]
    fn reset() {
        let prompt = "> ";
        let (terminal, mut editor) = get_terminal_and_editor(4, 10, Cursor::new(1, 0));
        let mut line = Line::new(
            prompt,
            &mut editor.buffer,
            &mut editor.terminal,
            &mut editor.history,
        );

        dbg!(terminal.get_cursor());

        let mut reset = line.reset();

        let probe = reset
            .start()
            .into_iter()
            .flat_map(|item| item.get_bytes().unwrap().to_vec())
            .collect::<Vec<u8>>();

        assert_eq!(probe, b"\x1b7\x1b[999;999H\x1b[6n\x1b8");

        let output = b"\x1b[91;45R"
            .iter()
            .copied()
            .flat_map(|b| reset.advance(b).unwrap().into_vec())
            .collect::<Vec<_>>();

        dbg!(terminal.get_cursor());

        assert_eq!(output, b"\r\x1b[J> \x1b[6n");

        let output = b"\x1b[2;3R"
            .iter()
            .copied()
            .flat_map(|b| {
                if let Some(output) = reset.advance(b) {
                    output.into_vec()
                } else {
                    Vec::new()
                }
            })
            .collect::<Vec<_>>();

        dbg!(terminal.get_cursor());

        assert_eq!(output, b"");

        assert_eq!(line.terminal.get_size(), (91, 45));
    }

    #[test]
    fn mock_editor() {
        let prompt = "> ";
        let (mut terminal, mut editor) = get_terminal_and_editor(4, 20, Cursor::new(1, 0));

        assert_eq!(terminal.get_cursor(), Cursor::new(1, 0));

        let line = editor.get_line(prompt, &mut terminal);

        dbg!(&line.terminal);
        assert_eq!(terminal.get_cursor(), line.terminal.get_cursor());
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

        dbg!(terminal.get_cursor());
        dbg!(&line.terminal);
        assert_eq!(terminal.get_cursor(), Cursor::new(1, 2));
        dbg!(terminal.screen_as_string());

        advance(&mut terminal, &mut line, "Hello, World!").unwrap();
        assert_eq!(terminal.get_cursor(), Cursor::new(1, 15));
        assert_eq!(terminal.screen_as_string(), "> Hello, World!");

        advance(&mut terminal, &mut line, CtrlU).unwrap();
        assert_eq!(terminal.get_cursor(), Cursor::new(1, 2));
        assert_eq!(terminal.screen_as_string(), "> ");

        advance(&mut terminal, &mut line, "Hello, World!").unwrap();
        assert_eq!(terminal.get_cursor(), Cursor::new(1, 15));
        assert_eq!(terminal.screen_as_string(), "> Hello, World!");
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

        advance(&mut terminal, &mut line, "Hello, World!").unwrap();
        assert_eq!(terminal.get_cursor(), Cursor::new(0, 15));
        assert_eq!(terminal.screen_as_string(), "> Hello, World!");
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
    fn slice_buffer() {
        let mut array = [0; 20];
        let mut terminal = MockTerminal::new(20, 80, Cursor::new(0, 0));
        let mut editor: Editor<_, NoHistory> =
            Editor::new(LineBuffer::from_slice(&mut array), NoHistory {});

        let mut line = editor.get_line("> ", &mut terminal);

        let input: String = (0..20).map(|_| "a").collect();

        advance(&mut terminal, &mut line, input.as_str()).unwrap();

        assert_eq!(advance(&mut terminal, &mut line, "a"), Err(()));

        assert_eq!(line.buffer.as_str(), input);

        advance(&mut terminal, &mut line, Backspace).unwrap();
    }

    #[test]
    fn history() {
        fn test<H: History>(history: H) {
            let mut terminal = MockTerminal::new(20, 80, Cursor::new(0, 0));
            let mut editor: Editor<_, H> = Editor::new(LineBuffer::new_unbounded(), history);

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

        test(UnboundedHistory::new());
        let mut buffer = [0; 128];
        test(SliceHistory::new(&mut buffer));
    }
}
