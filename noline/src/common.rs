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
    pub(crate) prompt: &'a str,
    _marker: PhantomData<(S, B)>,
}

impl<'a, B: Buffer, S: SyncAsync> NolineInitializer<'a, B, S> {
    pub fn new(prompt: &'a str) -> Self {
        Self {
            state: NolineInitializerState::New,
            parser: Parser::new(),
            prompt,
            _marker: PhantomData,
        }
    }

    pub(crate) fn init(&self) -> &'static [u8] {
        "\r\x1b[J\x1b7\x1b[6n\x1b[999;999H\x1b[6n\x1b8".as_bytes()
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
    pub(crate) buffer: LineBuffer<B>,
    terminal: Terminal,
    parser: Parser,
    prompt: &'a str,
    _marker: PhantomData<S>,
}

impl<'a, B: Buffer, S: SyncAsync> Noline<'a, B, S> {
    pub fn new(prompt: &'a str, terminal: Terminal) -> Self {
        Self {
            buffer: LineBuffer::new(),
            terminal,
            parser: Parser::new(),
            prompt,
            _marker: PhantomData,
        }
    }

    pub fn reset_line<'b>(&'b mut self) -> Output<'b, B> {
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

    pub(crate) fn input_byte<'b>(&'b mut self, byte: u8) -> Output<'b, B> {
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
                CarriageReturn => self.generate_output(Done),
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
                CSI::CUU(_) => self.generate_output(RingBell),
                CSI::CUD(_) => self.generate_output(RingBell),
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
    use std::sync::mpsc::channel;
    use std::vec::Vec;

    use crate::error::Error;
    use crate::sync::{Noline, NolineInitializer};
    use crate::terminal::Cursor;
    use crate::testlib::{csi, AsByteVec, MockTerminal};

    use super::*;

    fn advance<'a, B: Buffer>(
        terminal: &mut MockTerminal,
        noline: &mut Noline<'a, B>,
        input: impl AsByteVec,
    ) -> core::result::Result<(), ()> {
        terminal.bell = false;

        for input in input.as_byte_vec() {
            noline.advance::<_, (), ()>(input, |bytes| {
                for output in bytes {
                    terminal.advance(*output);
                }
                Ok(())
            });
        }

        assert_eq!(noline.terminal.get_cursor(), terminal.cursor);

        dbg!(terminal.screen_as_string());

        if terminal.bell {
            Err(())
        } else {
            Ok(())
        }
    }

    fn get_terminal_and_noline<'a>(
        prompt: &'a str,
        rows: usize,
        columns: usize,
        origin: Cursor,
    ) -> (MockTerminal, Noline<'a, Vec<u8>>) {
        let (tx, rx) = channel();

        let mut terminal = MockTerminal::new(rows, columns, origin);

        let mut noline = NolineInitializer::new(prompt)
            .initialize(
                || rx.try_recv().or_else(|_| Error::read_error(())),
                |bytes| {
                    for byte in bytes {
                        if let Some(bytes) = terminal.advance(*byte) {
                            for byte in bytes {
                                tx.send(byte).or_else(|_| Error::write_error(()))?;
                            }
                        }
                    }

                    Ok(())
                },
            )
            .unwrap();

        assert!(rx.try_recv().is_err());

        for item in noline.reset_line() {
            match item {
                crate::output::OutputItem::Slice(bytes) => {
                    for byte in bytes {
                        if let Some(bytes) = terminal.advance(*byte) {
                            for byte in bytes {
                                tx.send(byte).unwrap();
                            }
                        }
                    }
                }
                _ => unreachable!(),
            }
        }

        assert_eq!(terminal.get_cursor(), Cursor::new(origin.row, 2));
        assert_eq!(terminal.screen_as_string(), prompt);

        (terminal, noline)
    }

    #[test]
    fn movecursor() {
        let prompt = "> ";
        let (mut terminal, mut noline) = get_terminal_and_noline(prompt, 4, 10, Cursor::new(1, 0));

        advance(&mut terminal, &mut noline, "Hello, World!").unwrap();
        assert_eq!(terminal.get_cursor(), Cursor::new(2, 5));

        advance(&mut terminal, &mut noline, [csi::LEFT; 6]).unwrap();
        assert_eq!(terminal.get_cursor(), Cursor::new(1, 9));

        advance(&mut terminal, &mut noline, CtrlA).unwrap();

        assert_eq!(terminal.get_cursor(), Cursor::new(1, 2));

        assert!(advance(&mut terminal, &mut noline, csi::LEFT).is_err());

        assert_eq!(terminal.get_cursor(), Cursor::new(1, 2));

        advance(&mut terminal, &mut noline, CtrlE).unwrap();

        assert_eq!(terminal.get_cursor(), Cursor::new(2, 5));

        assert!(advance(&mut terminal, &mut noline, csi::RIGHT).is_err());

        assert_eq!(terminal.get_cursor(), Cursor::new(2, 5));

        advance(&mut terminal, &mut noline, csi::HOME).unwrap();

        assert_eq!(terminal.get_cursor(), Cursor::new(1, 2));

        advance(&mut terminal, &mut noline, csi::END).unwrap();

        assert_eq!(terminal.get_cursor(), Cursor::new(2, 5));
    }

    #[test]
    fn cursor_scroll() {
        let prompt = "> ";
        let (mut terminal, mut noline) = get_terminal_and_noline(prompt, 4, 10, Cursor::new(3, 0));

        advance(&mut terminal, &mut noline, "23456789").unwrap();
        assert_eq!(terminal.get_cursor(), Cursor::new(3, 0));
    }

    #[test]
    fn clear_line() {
        let prompt = "> ";
        let (mut terminal, mut noline) = get_terminal_and_noline(prompt, 4, 20, Cursor::new(1, 0));

        advance(&mut terminal, &mut noline, "Hello, World!").unwrap();
        assert_eq!(terminal.get_cursor(), Cursor::new(1, 15));
        assert_eq!(terminal.screen_as_string(), "> Hello, World!");

        advance(&mut terminal, &mut noline, CtrlU).unwrap();
        assert_eq!(terminal.get_cursor(), Cursor::new(1, 2));
        assert_eq!(terminal.screen_as_string(), "> ");
    }

    #[test]
    fn clear_screen() {
        let prompt = "> ";
        let (mut terminal, mut noline) = get_terminal_and_noline(prompt, 4, 20, Cursor::new(1, 0));

        advance(&mut terminal, &mut noline, "Hello, World!").unwrap();
        assert_eq!(terminal.get_cursor(), Cursor::new(1, 15));
        assert_eq!(terminal.screen_as_string(), "> Hello, World!");

        advance(&mut terminal, &mut noline, CtrlL).unwrap();
        assert_eq!(terminal.get_cursor(), Cursor::new(0, 2));
        assert_eq!(terminal.screen_as_string(), "> ");
    }

    #[test]
    fn scroll() {
        let prompt = "> ";
        let (mut terminal, mut noline) = get_terminal_and_noline(prompt, 4, 10, Cursor::new(0, 0));

        advance(&mut terminal, &mut noline, "aaaaaaaa").unwrap();
        advance(&mut terminal, &mut noline, "bbbbbbbbbb").unwrap();
        advance(&mut terminal, &mut noline, "cccccccccc").unwrap();
        advance(&mut terminal, &mut noline, "ddddddddd").unwrap();

        assert_eq!(terminal.get_cursor(), Cursor::new(3, 9));

        assert_eq!(
            terminal.screen_as_string(),
            "> aaaaaaaa\nbbbbbbbbbb\ncccccccccc\nddddddddd"
        );

        advance(&mut terminal, &mut noline, "d").unwrap();

        assert_eq!(terminal.get_cursor(), Cursor::new(3, 0));

        assert_eq!(
            terminal.screen_as_string(),
            "bbbbbbbbbb\ncccccccccc\ndddddddddd"
        );

        advance(&mut terminal, &mut noline, "eeeeeeeeee").unwrap();

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
        let (mut terminal, mut noline) = get_terminal_and_noline(prompt, 4, 10, Cursor::new(0, 0));

        advance(&mut terminal, &mut noline, "æøå").unwrap();
        assert_eq!(terminal.screen_as_string(), "> æøå");
        assert_eq!(terminal.get_cursor(), Cursor::new(0, 5));

        assert!(advance(&mut terminal, &mut noline, CtrlT).is_err());

        assert_eq!(terminal.screen_as_string(), "> æøå");
        assert_eq!(terminal.get_cursor(), Cursor::new(0, 5));

        advance(&mut terminal, &mut noline, csi::LEFT).unwrap();

        assert_eq!(terminal.get_cursor(), Cursor::new(0, 4));

        advance(&mut terminal, &mut noline, CtrlT).unwrap();

        assert_eq!(noline.buffer.as_str(), "æåø");
        assert_eq!(terminal.screen_as_string(), "> æåø");
        assert_eq!(terminal.get_cursor(), Cursor::new(0, 4));

        advance(&mut terminal, &mut noline, CtrlA).unwrap();

        assert_eq!(terminal.get_cursor(), Cursor::new(0, 2));

        assert!(advance(&mut terminal, &mut noline, CtrlT).is_err());
        assert_eq!(terminal.screen_as_string(), "> æåø");
    }

    #[test]
    fn erase_after_cursor() {
        let prompt = "> ";
        let (mut terminal, mut noline) = get_terminal_and_noline(prompt, 4, 10, Cursor::new(0, 0));

        advance(&mut terminal, &mut noline, "rm -rf /").unwrap();
        assert_eq!(terminal.get_cursor(), Cursor::new(1, 0));
        assert_eq!(terminal.screen_as_string(), "> rm -rf /");

        advance(&mut terminal, &mut noline, CtrlA).unwrap();
        advance(&mut terminal, &mut noline, [CtrlF; 3]).unwrap();

        assert_eq!(terminal.get_cursor(), Cursor::new(0, 5));

        advance(&mut terminal, &mut noline, CtrlK).unwrap();
        assert_eq!(noline.buffer.as_str(), "rm ");
        assert_eq!(terminal.get_cursor(), Cursor::new(0, 5));
        assert_eq!(terminal.screen_as_string(), "> rm ");
    }

    #[test]
    fn delete_previous_word() {
        let prompt = "> ";
        let (mut terminal, mut noline) = get_terminal_and_noline(prompt, 1, 40, Cursor::new(0, 0));

        advance(&mut terminal, &mut noline, "rm file1 file2 file3").unwrap();
        assert_eq!(terminal.screen_as_string(), "> rm file1 file2 file3");

        advance(&mut terminal, &mut noline, [CtrlB; 5]).unwrap();

        advance(&mut terminal, &mut noline, CtrlW).unwrap();
        assert_eq!(terminal.get_cursor(), Cursor::new(0, 11));
        assert_eq!(noline.buffer.as_str(), "rm file1 file3");
        assert_eq!(terminal.screen_as_string(), "> rm file1 file3");

        advance(&mut terminal, &mut noline, CtrlW).unwrap();
        assert_eq!(terminal.screen_as_string(), "> rm file3");
        assert_eq!(terminal.get_cursor(), Cursor::new(0, 5));
    }

    #[test]
    fn delete() {
        let prompt = "> ";
        let (mut terminal, mut noline) = get_terminal_and_noline(prompt, 1, 40, Cursor::new(0, 0));

        assert_eq!(terminal.get_cursor(), Cursor::new(0, 2));
        assert_eq!(terminal.screen_as_string(), "> ");
        advance(&mut terminal, &mut noline, "abcde").unwrap();

        advance(&mut terminal, &mut noline, CtrlD).unwrap_err();

        advance(&mut terminal, &mut noline, CtrlA).unwrap();

        advance(&mut terminal, &mut noline, CtrlD).unwrap();
        assert_eq!(noline.buffer.as_str(), "bcde");
        assert_eq!(terminal.screen_as_string(), "> bcde");

        advance(&mut terminal, &mut noline, [csi::RIGHT; 3]).unwrap();
        advance(&mut terminal, &mut noline, CtrlD).unwrap();
        assert_eq!(noline.buffer.as_str(), "bcd");
        assert_eq!(terminal.screen_as_string(), "> bcd");

        advance(&mut terminal, &mut noline, CtrlD).unwrap_err();

        advance(&mut terminal, &mut noline, CtrlA).unwrap();

        advance(&mut terminal, &mut noline, csi::DELETE).unwrap();
        assert_eq!(noline.buffer.as_str(), "cd");
        assert_eq!(terminal.screen_as_string(), "> cd");

        advance(&mut terminal, &mut noline, csi::DELETE).unwrap();
        assert_eq!(noline.buffer.as_str(), "d");
        assert_eq!(terminal.screen_as_string(), "> d");
    }

    #[test]
    fn backspace() {
        let prompt = "> ";
        let (mut terminal, mut noline) = get_terminal_and_noline(prompt, 1, 40, Cursor::new(0, 0));

        assert!(advance(&mut terminal, &mut noline, Backspace).is_err());

        assert_eq!(terminal.get_cursor(), Cursor::new(0, 2));
        assert_eq!(terminal.screen_as_string(), "> ");
        advance(&mut terminal, &mut noline, "hello").unwrap();

        advance(&mut terminal, &mut noline, Backspace).unwrap();
        assert_eq!(noline.buffer.as_str(), "hell");
        assert_eq!(terminal.screen_as_string(), "> hell");

        advance(&mut terminal, &mut noline, [csi::LEFT; 2]).unwrap();
        advance(&mut terminal, &mut noline, Backspace).unwrap();
        assert_eq!(noline.buffer.as_str(), "hll");
        assert_eq!(terminal.screen_as_string(), "> hll");

        advance(&mut terminal, &mut noline, CtrlA).unwrap();
        advance(&mut terminal, &mut noline, Backspace).unwrap_err();
    }
}
