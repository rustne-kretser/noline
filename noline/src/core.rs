//! Core library containing the components for building an editor.
//!
//! Use [`Line`] to initialize the terminal and read a single line.

use crate::editor::Error;
use crate::history::{History, HistoryNavigator};
use crate::input::{Action, ControlCharacter::*, Parser, CSI};
use crate::line_buffer::Buffer;
use crate::line_buffer::LineBuffer;
use crate::output::CursorMove;
use crate::output::{Output, OutputAction};
use crate::terminal::{Cursor, Terminal};

use OutputAction::*;

enum LineState {
    New,
    GetSize { cursor: usize, parser: Parser },
    GetPosition { cursor: usize, parser: Parser },
    Suspended(usize),
    Editing,
    Finished,
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
            len: parts.clone().map(|part| part.chars().count()).sum(),
            parts,
        }
    }

    /// Number of Unicode scalar values in the prompt.
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
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

/// One editing session. Start the line, then drain and write each operation's output.
pub struct Line<'a, B: Buffer, H: History, I> {
    buffer: &'a mut LineBuffer<B>,
    terminal: &'a mut Terminal,
    parser: Parser,
    prompt: Prompt<I>,
    nav: &'a mut HistoryNavigator<H>,
    state: LineState,
}

impl<'a, 'item, B: Buffer, H: History, I> Line<'a, B, H, I>
where
    I: Iterator<Item = &'item str> + Clone,
{
    pub(crate) fn new(
        prompt: impl Into<Prompt<I>>,
        buffer: &'a mut LineBuffer<B>,
        terminal: &'a mut Terminal,
        nav: &'a mut HistoryNavigator<H>,
    ) -> Self {
        Self {
            buffer,
            terminal,
            parser: Parser::new(),
            prompt: prompt.into(),
            nav,
            state: LineState::New,
        }
    }

    /// Change the prompt between lines or while suspended. No output is generated.
    pub fn set_prompt(&mut self, prompt: impl Into<Prompt<I>>) -> Result<(), Error> {
        if !matches!(
            self.state,
            LineState::New | LineState::Finished | LineState::Suspended(_)
        ) {
            return Err(Error::InvalidState);
        }
        self.prompt = prompt.into();
        Ok(())
    }

    /// Clear the input and probe terminal size and cursor position.
    /// The terminal must answer the queries before ordinary input is accepted.
    pub fn start(&mut self) -> Output<'_, B, I> {
        self.buffer.truncate();
        self.parser = Parser::new();
        self.nav.reset();
        self.state = LineState::GetSize {
            cursor: 0,
            parser: Parser::new(),
        };
        self.generate_output(ProbeSize)
    }

    /// Clear the input and print the prompt with known terminal geometry.
    /// `row` is the terminal's current zero-based row, available for the prompt.
    /// Invalid dimensions or an out-of-bounds row leave the editor unchanged.
    pub fn start_at(
        &mut self,
        rows: usize,
        columns: usize,
        row: usize,
    ) -> Result<Output<'_, B, I>, Error> {
        *self.terminal = Terminal::try_new(rows, columns, row).ok_or(Error::InvalidGeometry)?;
        self.buffer.truncate();
        self.parser = Parser::new();
        self.nav.reset();
        self.state = LineState::Editing;
        Ok(self.generate_output(Restore(0)))
    }

    /// Clear the displayed draft and hand the terminal to the application.
    /// Drain the output before printing. The draft, cursor, parser and history
    /// navigation are retained. Do not feed input until resuming.
    pub fn suspend(&mut self) -> Result<Output<'_, B, I>, Error> {
        if !matches!(self.state, LineState::Editing) {
            return Err(Error::InvalidState);
        }
        self.state = LineState::Suspended(self.current_position());
        Ok(self.generate_output(Suspend))
    }

    /// Probe the terminal and restore suspended editing.
    /// Leave a line available for the prompt and restore the terminal's normal
    /// editing modes first. As with `start()`, queries must be answered before
    /// ordinary input is fed. Drain all output before continuing.
    pub fn resume(&mut self) -> Result<Output<'_, B, I>, Error> {
        let LineState::Suspended(cursor) = self.state else {
            return Err(Error::InvalidState);
        };
        self.state = LineState::GetSize {
            cursor,
            parser: Parser::new(),
        };
        Ok(self.generate_output(ProbeSize))
    }

    /// Restore suspended editing with known terminal geometry.
    /// The application must leave the cursor at column zero of `row`, with that
    /// line available for the prompt. Invalid geometry leaves editing suspended.
    pub fn resume_at(
        &mut self,
        rows: usize,
        columns: usize,
        row: usize,
    ) -> Result<Output<'_, B, I>, Error> {
        let LineState::Suspended(cursor) = self.state else {
            return Err(Error::InvalidState);
        };
        *self.terminal = Terminal::try_new(rows, columns, row).ok_or(Error::InvalidGeometry)?;
        self.state = LineState::Editing;
        Ok(self.generate_output(Restore(cursor)))
    }

    /// Current input, including a submitted line.
    pub fn as_str(&self) -> &str {
        self.buffer.as_str()
    }

    /// End the session and borrow its input from the editor's buffer.
    pub fn into_str(self) -> &'a str {
        self.buffer.as_str()
    }

    /// Cursor position as a UTF-8 byte offset, available while editing.
    pub fn cursor(&self) -> Option<usize> {
        if !matches!(self.state, LineState::Editing) {
            return None;
        }
        Some(self.buffer.get_byte_position(self.current_position()))
    }

    /// Replace a UTF-8 byte range and place the cursor after the inserted text.
    /// Cancels incomplete input sequences, including when the replacement is rejected.
    /// Invalid ranges, control characters, or insufficient capacity ring the bell
    /// without modifying the line. Fully drain the returned output before editing again.
    pub fn replace(
        &mut self,
        range: core::ops::Range<usize>,
        text: &str,
    ) -> Result<Output<'_, B, I>, Error> {
        if !matches!(self.state, LineState::Editing) {
            return Err(Error::InvalidState);
        }
        self.parser = Parser::new();
        let start = range.start;
        if text.chars().any(char::is_control) || self.buffer.replace_range(range, text).is_err() {
            return Ok(self.generate_output(RingBell));
        }
        let cursor = self.as_str()[..start + text.len()].chars().count();
        Ok(self.generate_output(RedrawAt(cursor)))
    }

    fn generate_output(&mut self, action: OutputAction) -> Output<'_, B, I> {
        Output::new(&self.prompt, self.buffer, self.terminal, action)
    }

    fn current_position(&self) -> usize {
        let pos = self.terminal.current_offset() as usize;
        pos - self.prompt.len()
    }

    fn delete_forward(&mut self) -> Output<'_, B, I> {
        let pos = self.current_position();
        let action = if self.buffer.as_str().chars().nth(pos).is_some() {
            self.buffer.delete(pos);
            EraseAndPrintBuffer
        } else {
            RingBell
        };
        self.generate_output(action)
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

        let action = entry
            .and_then(|entry| self.buffer.restore_history(entry))
            .map_or(RingBell, |_| ClearAndPrintBuffer);
        self.generate_output(action)
    }

    fn history_move_down(&mut self) -> Output<'_, B, I> {
        let entry = if self.nav.is_active() {
            self.nav.move_down()
        } else {
            return self.generate_output(RingBell);
        };

        let action = if let Ok(entry) = entry {
            self.buffer
                .restore_history(entry)
                .map_or(RingBell, |_| ClearAndPrintBuffer)
        } else {
            self.nav.reset();
            self.buffer.truncate();
            ClearAndPrintBuffer
        };
        self.generate_output(action)
    }

    /// Feed a byte and consume the returned output before feeding another.
    /// Submission and abort are reported by [`crate::editor::OutputItem`].
    /// Input while suspended is rejected without consumption. Input before
    /// starting or after completion is ignored.
    pub fn advance(&mut self, byte: u8) -> Result<Output<'_, B, I>, Error> {
        if matches!(self.state, LineState::Suspended(_)) {
            return Err(Error::InvalidState);
        }
        Ok(self.advance_input(byte))
    }

    fn advance_input(&mut self, byte: u8) -> Output<'_, B, I> {
        if matches!(self.state, LineState::New | LineState::Finished) {
            return self.generate_output(Nothing);
        }
        let action = match &mut self.state {
            LineState::GetSize { parser, .. } | LineState::GetPosition { parser, .. } => {
                parser.advance(byte)
            }
            _ => self.parser.advance(byte),
        };

        if !matches!(self.state, LineState::Editing) {
            return match (action, &self.state) {
                (Action::Ignore, _) => self.generate_output(Nothing),
                (
                    Action::ControlSequenceIntroducer(CSI::CPR(rows, columns)),
                    LineState::GetSize { cursor, .. },
                ) if Terminal::try_new(rows, columns, 0).is_some() => {
                    self.terminal.resize(rows, columns);
                    self.state = LineState::GetPosition {
                        cursor: *cursor,
                        parser: Parser::new(),
                    };
                    self.generate_output(Locate)
                }
                (
                    Action::ControlSequenceIntroducer(CSI::CPR(row, column)),
                    LineState::GetPosition { cursor, .. },
                ) if self.terminal.contains(row - 1, column - 1) => {
                    let cursor = *cursor;
                    self.terminal.reset(Cursor::new(row - 1, 0));
                    self.state = LineState::Editing;
                    self.generate_output(Restore(cursor))
                }
                _ => {
                    self.state = LineState::Finished;
                    self.generate_output(Abort)
                }
            };
        }

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
                CtrlC => {
                    self.state = LineState::Finished;
                    self.generate_output(Abort)
                }
                CtrlD if self.buffer.len() == 0 => {
                    self.state = LineState::Finished;
                    self.generate_output(Abort)
                }
                CtrlD => self.delete_forward(),
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

                    self.state = LineState::Finished;
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
                CSI::Delete => self.delete_forward(),
                CSI::End => self.generate_output(MoveCursor(CursorMove::End)),
                CSI::CPR(_, _) => self.generate_output(Nothing),
                CSI::Unknown(_) => self.generate_output(RingBell),
                CSI::CUU(_) => self.history_move_up(),
                CSI::CUD(_) => self.history_move_down(),
                CSI::CUP(_, _) => self.generate_output(RingBell),
                CSI::ED(_) => self.generate_output(RingBell),
                CSI::DSR => self.generate_output(RingBell),
                CSI::SU(_) => self.generate_output(RingBell),
                CSI::SD(_) => self.generate_output(RingBell),
                CSI::Invalid => self.generate_output(RingBell),
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

    #[test]
    fn delete_at_unicode_end() {
        for key in ["\x04", "\x1b[3~"] {
            let (mut terminal, mut editor) = get_terminal_and_editor(4, 80, Cursor::new(0, 0));
            let mut line = editor.get_line("> ", &mut terminal);
            advance(&mut terminal, &mut line, "é").unwrap();
            assert_eq!(advance(&mut terminal, &mut line, key), Err(()));
            advance(&mut terminal, &mut line, "\x02").unwrap();
            advance(&mut terminal, &mut line, key).unwrap();
            advance(&mut terminal, &mut line, "x").unwrap();
            assert_eq!(line.buffer.as_str(), "x");
            assert_eq!(terminal.current_line_as_string(), "> x");
        }
    }

    #[test]
    fn suspend_preserves_editing() {
        for columns in [8, 20] {
            for probe in [false, true] {
                let mut terminal = MockTerminal::new(4, columns, Cursor::new(3, 0));
                let mut editor = Editor::new(LineBuffer::new_unbounded(), UnboundedHistory::new());
                editor
                    .nav
                    .history
                    .load_entries(["older", "abcdefgh"].into_iter());
                let line = &mut editor.get_line("> ", &mut terminal);
                advance(&mut terminal, line, "\x10\x1b[D\x1b[").unwrap();
                for byte in line.suspend().unwrap().into_vec() {
                    terminal.advance(byte);
                }
                assert!(matches!(line.suspend(), Err(Error::InvalidState)));
                assert!(matches!(line.advance(b'x'), Err(Error::InvalidState)));
                assert!(matches!(line.replace(0..1, "x"), Err(Error::InvalidState)));
                assert!(matches!(
                    line.resume_at(4, 0, 0),
                    Err(Error::InvalidGeometry)
                ));
                for byte in b"first\r\n12345678\r\n\r\nlast\r\n" {
                    terminal.advance(*byte);
                }
                let mut output = if probe {
                    line.resume().unwrap().into_vec()
                } else {
                    line.resume_at(4, columns, terminal.cursor.row)
                        .unwrap()
                        .into_vec()
                };
                while !output.is_empty() {
                    output = output
                        .into_iter()
                        .filter_map(|b| terminal.advance(b))
                        .flatten()
                        .flat_map(|b| line.advance(b).unwrap().into_vec())
                        .collect();
                }
                assert_eq!(line.as_str(), "abcdefgh");
                assert_eq!(line.cursor(), Some(7));
                assert_eq!(
                    terminal.screen_as_string(),
                    if columns == 8 {
                        "last\n> abcdef\ngh"
                    } else {
                        "12345678\nlast\n> abcdefgh"
                    }
                );
                advance(&mut terminal, line, "DX").unwrap();
                assert_eq!(line.as_str(), "abcdefXgh");
                advance(&mut terminal, line, "\x10").unwrap();
                assert_eq!(line.as_str(), "older");
            }
        }
    }

    #[test]
    fn resume_with_new_prompt_and_geometry() {
        let mut storage = Editor::new(LineBuffer::new_unbounded(), NoHistory {});
        let mut editor = Line::new(
            ["old", "> "].into_iter(),
            &mut storage.buffer,
            &mut storage.terminal,
            &mut storage.nav,
        );
        editor.start_at(4, 20, 0).unwrap().into_vec();
        for byte in b"abc\x02" {
            editor.advance(*byte).unwrap().into_vec();
        }
        assert!(editor.set_prompt(["bad", "> "].into_iter()).is_err());
        editor.suspend().unwrap().into_vec();
        editor.set_prompt(["new", ": "].into_iter()).unwrap();
        let mut terminal = MockTerminal::new(4, 6, Cursor::new(3, 0));
        for byte in editor.resume_at(4, 6, 3).unwrap().into_vec() {
            terminal.advance(byte);
        }
        assert_eq!(terminal.screen_as_string(), "new: a\nbc");
        assert_eq!(editor.cursor(), Some(2));
        assert_eq!(editor.terminal.get_cursor(), terminal.cursor);
    }

    #[test]
    fn oversized_history() {
        let mut storage = [0; 2];
        let mut entries = [0; 32];
        let mut history = SliceHistory::new(&mut entries);
        history.load_entries(["ok", "too long"].into_iter());
        let mut editor = Editor::new(LineBuffer::from_slice(&mut storage), history);
        let mut terminal = MockTerminal::new(4, 80, Cursor::new(0, 0));
        let mut line = editor.get_line("> ", &mut terminal);
        assert_eq!(advance(&mut terminal, &mut line, "\x10"), Err(()));
        assert_eq!(line.buffer.as_str(), "");
        advance(&mut terminal, &mut line, "\x10").unwrap();
        assert_eq!(line.buffer.as_str(), "ok");
        assert_eq!(advance(&mut terminal, &mut line, "\x0e"), Err(()));
        assert_eq!(line.buffer.as_str(), "ok");
    }

    #[test]
    fn submit_below_wrapped_draft() {
        let (mut terminal, mut editor) = get_terminal_and_editor(4, 4, Cursor::new(0, 0));
        let mut line = editor.get_line("> ", &mut terminal);
        advance(&mut terminal, &mut line, "abcdefgh\x01\r").unwrap();
        assert_eq!(terminal.get_cursor(), Cursor::new(3, 0));
        assert_eq!(terminal.screen_as_string(), "> ab\ncdef\ngh");
    }

    #[test]
    fn multibyte_prompt() {
        let (mut terminal, mut editor) = get_terminal_and_editor(4, 80, Cursor::new(0, 0));
        let mut line = editor.get_line("é> ", &mut terminal);
        advance(&mut terminal, &mut line, "ab\x02!").unwrap();
        assert_eq!(line.buffer.as_str(), "a!b");
        assert_eq!(terminal.current_line_as_string(), "é> a!b");
    }

    #[test]
    fn replace_utf8() {
        for (columns, row, screen) in [
            (80, 0, "> get /caféteria/0"),
            (8, 2, "> get /c\naféteria\n/0"),
        ] {
            let (mut terminal, mut editor) =
                get_terminal_and_editor(3, columns, Cursor::new(row, 0));
            let line = &mut editor.get_line("> ", &mut terminal);
            advance(&mut terminal, line, "get /cafe/0").unwrap();
            for byte in line.replace(5..9, "caféteria").unwrap().into_vec() {
                terminal.advance(byte);
            }
            assert_eq!(line.replace(0..0, "\r").unwrap().into_vec(), b"\x07");
            assert_eq!(line.as_str(), "get /caféteria/0");
            assert_eq!(line.cursor(), Some(15));
            assert_eq!(terminal.screen_as_string(), screen);
            advance(&mut terminal, line, "!").unwrap();
            assert_eq!(line.as_str(), "get /caféteria!/0");
        }
    }

    #[test]
    fn replacement_cancels_partial_input() {
        for (prefix, text, expected) in [
            (b"\x1b".as_slice(), "set ", "set /"),
            (b"\x1b[", "set ", "set /"),
            (b"\xc3", "set ", "set /"),
            (b"\xc3", "\r", "se/"),
        ] {
            let (mut terminal, mut editor) = get_terminal_and_editor(4, 80, Cursor::new(0, 0));
            let line = &mut editor.get_line("> ", &mut terminal);
            advance(&mut terminal, line, "se").unwrap();
            for &byte in prefix {
                line.advance(byte).unwrap().into_vec();
            }
            for byte in line.replace(0..2, text).unwrap().into_vec() {
                terminal.advance(byte);
            }
            advance(&mut terminal, line, "/").unwrap();
            assert_eq!(line.as_str(), expected);
            assert_eq!(terminal.current_line_as_string(), format!("> {expected}"));
        }
    }

    struct Editor<B: Buffer, H: History> {
        buffer: LineBuffer<B>,
        terminal: Terminal,
        nav: HistoryNavigator<H>,
    }

    impl<B: Buffer, H: History> Editor<B, H> {
        fn new(buffer: LineBuffer<B>, history: H) -> Self {
            let terminal = Terminal::default();

            Self {
                buffer,
                terminal,
                nav: HistoryNavigator::new(history),
            }
        }

        fn get_line(
            &mut self,
            prompt: &'static str,
            mockterm: &mut MockTerminal,
        ) -> Line<'_, B, H, StrIter<'static>> {
            let cursor = mockterm.get_cursor();
            let mut line = Line::new(prompt, &mut self.buffer, &mut self.terminal, &mut self.nav);

            let mut reset_start: Vec<u8> = line
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
                    .into_iter()
                    .flat_map(|b| line.advance(b).unwrap().into_vec())
                    .collect();
            }

            assert_eq!(mockterm.current_line_as_string(), prompt);
            assert_eq!(
                mockterm.get_cursor(),
                Cursor::new(cursor.row, prompt.chars().count())
            );

            line
        }
    }

    fn advance<'a, B: Buffer, H: History>(
        terminal: &mut MockTerminal,
        noline: &mut Line<'_, B, H, StrIter<'a>>,
        input: impl ToByteVec,
    ) -> core::result::Result<(), ()> {
        terminal.bell = false;

        for input in input.to_byte_vec() {
            for item in noline.advance(input).unwrap() {
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
            &mut editor.nav,
        );

        dbg!(terminal.get_cursor());

        let probe = line
            .start()
            .into_iter()
            .flat_map(|item| item.get_bytes().unwrap().to_vec())
            .collect::<Vec<u8>>();

        assert_eq!(probe, b"\x1b7\x1b[999;999H\x1b[6n\x1b8");

        let output = b"\x1b[91;45R"
            .iter()
            .copied()
            .flat_map(|b| line.advance(b).unwrap().into_vec())
            .collect::<Vec<_>>();

        dbg!(terminal.get_cursor());

        assert_eq!(output, b"\x1b[6n");

        let output = b"\x1b[2;3R"
            .iter()
            .copied()
            .flat_map(|b| line.advance(b).unwrap().into_vec())
            .collect::<Vec<_>>();

        dbg!(terminal.get_cursor());

        assert_eq!(output, b"\r\x1b[J> \x1b[2;3H");

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
    fn configured_session() {
        let mut storage = [0; 32];
        let mut buffer = LineBuffer::from_slice(&mut storage);
        let mut state = Terminal::default();
        let mut history = HistoryNavigator::new(NoHistory {});
        let prompt = String::from("> ");
        let mut line = Line::new(prompt.as_str(), &mut buffer, &mut state, &mut history);
        for input in [b'a', 0x1b] {
            assert!(line.start_at(4, 0, 2).is_err());
            line.start_at(4, 20, 0).unwrap().into_vec();
            line.advance(input).unwrap().into_iter().next();
            let mut terminal = MockTerminal::new(4, 20, Cursor::new(2, 0));
            for byte in line.start_at(4, 20, 2).unwrap().into_vec() {
                assert!(terminal.advance(byte).is_none());
            }
            advance(&mut terminal, &mut line, "ab\x02X").unwrap();
            assert_eq!(line.cursor(), Some(2));
            assert_eq!(terminal.current_line_as_string(), "> aXb");
        }
        let input = line.into_str();
        drop(prompt);
        assert_eq!(input, "aXb");
    }

    #[test]
    fn invalid_position() {
        let mut buffer = LineBuffer::new_unbounded();
        let mut state = Terminal::default();
        let mut history = HistoryNavigator::new(NoHistory {});
        let mut line = Line::new("> ", &mut buffer, &mut state, &mut history);
        line.start().into_vec();
        let event = b"\x1b[4;20R\x1b[5;1R"
            .iter()
            .flat_map(|&b| {
                line.advance(b)
                    .unwrap()
                    .into_iter()
                    .filter_map(|item| item.event())
                    .collect::<Vec<_>>()
            })
            .last();
        assert_eq!(event, Some(crate::editor::Event::Aborted));
        line.start_at(4, 20, 0).unwrap().into_vec();
        line.advance(b'x').unwrap().into_vec();
        assert_eq!(line.as_str(), "x");
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
