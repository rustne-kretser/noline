use crate::history::{History, HistoryNavigator};
use crate::input::{Action, ControlCharacter::*, Parser, CSI};
use crate::line_buffer::Buffer;
use crate::line_buffer::LineBuffer;
use crate::output::CursorMove;
use crate::output::{Output, OutputAction};
use crate::terminal::{Cursor, Terminal};

use super::{Prompt, ResetHandle};
use OutputAction::*;

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
    pub(super) buffer: &'a mut LineBuffer<B>,
    pub(super) terminal: &'a mut Terminal,
    pub(super) parser: Parser,
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

    pub(super) fn generate_output(&mut self, action: OutputAction) -> Output<'_, B, I> {
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
                CSI::Invalid => self.generate_output(RingBell),
            },
            Action::EscapeSequence(_) => self.generate_output(RingBell),
            Action::Ignore => self.generate_output(Nothing),
            Action::InvalidUtf8 => self.generate_output(RingBell),
        }
    }
}
