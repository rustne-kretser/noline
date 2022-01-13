#![no_std]

#[cfg(any(test, feature = "std"))]
#[macro_use]
extern crate std;

mod input;
pub mod line_buffer;
mod utf8;

use crate::input::{Action, ControlCharacter::*, Parser, CSI};
use crate::line_buffer::{CursorMove, LineBuffer};

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum Instruction {
    PrintPrompt,
    PrintBuffer,
    PrintBufferFromPos,
    PrintBufferToPos,
    ClearScreen,
    EraseLine,
    EraseFromCursor,
    MoveCursorBack,
    MoveCursorForward,
    MoveCursorToEdge,
    SaveCursor,
    RestoreCursor,
    Bell,
    Newline,
    DeviceStatusReport,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Status {
    Continue(&'static [Instruction]),
    ContinueWithPosition(&'static [Instruction], usize),
    Done(&'static [Instruction]),
    Abort(&'static [Instruction]),
}

impl Status {
    fn args(
        self,
    ) -> (
        &'static [Instruction],
        Option<usize>,
        Option<Result<(), ()>>,
    ) {
        match self {
            Continue(instructions) => (instructions, None, None),
            ContinueWithPosition(instructions, position) => (instructions, Some(position), None),
            Done(instructions) => (instructions, None, Some(Ok(()))),
            Abort(instructions) => (instructions, None, Some(Err(()))),
        }
    }
}

use Instruction::*;
use Status::*;

pub struct Noline<'a, LB: LineBuffer> {
    buffer: &'a mut LB,
    parser: Parser,
    prompt: &'a [u8],
    line_width: Option<usize>,
}

impl<'a, LB: LineBuffer> Noline<'a, LB> {
    pub fn new(line_buffer: &'a mut LB, prompt: &'a [u8]) -> Self {
        Self {
            buffer: line_buffer,
            parser: Parser::new(),
            prompt,
            line_width: None,
        }
    }

    pub fn get_instruction(&mut self, instruction: Instruction, position: Option<usize>) -> &[u8] {
        match instruction {
            PrintPrompt => self.prompt,
            PrintBuffer => self.buffer.as_slice(),
            PrintBufferFromPos => {
                let pos = position.unwrap();
                &self.buffer.as_slice()[pos..]
            }
            PrintBufferToPos => {
                let pos = position.unwrap();
                &self.buffer.as_slice()[0..pos]
            }
            ClearScreen => "\x1b[2J\x1b[1;1H".as_bytes(),
            EraseLine => "\x1b[2K\r".as_bytes(),
            EraseFromCursor => "\x1b[K".as_bytes(),
            MoveCursorBack => "\x1b[D".as_bytes(),
            MoveCursorForward => "\x1b[C".as_bytes(),
            MoveCursorToEdge => "\x1b[0;999H".as_bytes(),
            SaveCursor => "\x1b7".as_bytes(),
            RestoreCursor => "\x1b8".as_bytes(),
            Bell => "\x07".as_bytes(),
            Newline => "\n\r".as_bytes(),
            DeviceStatusReport => "\x1b[6n".as_bytes(),
        }
    }

    pub fn init(&self) -> Status {
        Continue(&[
            SaveCursor,
            MoveCursorToEdge,
            DeviceStatusReport,
            RestoreCursor,
            EraseLine,
            PrintPrompt,
        ])
    }

    fn move_cursor(&mut self, cursor_move: CursorMove) -> Status {
        if self.buffer.move_cursor(cursor_move).is_ok() {
            match cursor_move {
                CursorMove::Back => Continue(&[MoveCursorBack]),
                CursorMove::Forward => Continue(&[MoveCursorForward]),
                CursorMove::Start => Continue(&[
                    EraseLine,
                    PrintPrompt,
                    SaveCursor,
                    PrintBuffer,
                    RestoreCursor,
                ]),
                CursorMove::End => Continue(&[EraseLine, PrintPrompt, PrintBuffer]),
            }
        } else {
            Continue(&[Bell])
        }
    }

    pub fn advance(&mut self, byte: u8) -> Status {
        let action = self.parser.advance(byte);

        #[cfg(test)]
        dbg!(action);

        match action {
            Action::Print(c) => {
                let pos = self.buffer.get_position();

                if self.buffer.insert_bytes(c.as_bytes()).is_ok() {
                    ContinueWithPosition(
                        &[
                            SaveCursor,
                            PrintBufferFromPos,
                            RestoreCursor,
                            MoveCursorForward,
                        ],
                        pos,
                    )
                } else {
                    Continue(&[Bell])
                }
            }
            Action::ControlCharacter(c) => match c {
                CtrlA => self.move_cursor(CursorMove::Start),
                CtrlB => self.move_cursor(CursorMove::Back),
                CtrlC => Abort(&[Newline]),
                CtrlD => {
                    if self.buffer.buffer_len() > 0 {
                        let pos = self.buffer.get_position();

                        if self.buffer.delete().is_ok() {
                            ContinueWithPosition(
                                &[
                                    SaveCursor,
                                    EraseFromCursor,
                                    PrintBufferFromPos,
                                    RestoreCursor,
                                ],
                                pos,
                            )
                        } else {
                            Continue(&[Bell])
                        }
                    } else {
                        Abort(&[Newline])
                    }
                }
                CtrlE => self.move_cursor(CursorMove::End),
                CtrlF => self.move_cursor(CursorMove::Forward),
                CtrlK => {
                    self.buffer.delete_after();
                    Continue(&[EraseFromCursor])
                }
                CtrlL => {
                    self.buffer.update_position(0);
                    self.buffer.delete_after();
                    Continue(&[ClearScreen, PrintPrompt])
                }
                CtrlT => {
                    self.buffer.swap_previous();
                    Continue(&[
                        SaveCursor,
                        EraseLine,
                        PrintPrompt,
                        PrintBuffer,
                        RestoreCursor,
                    ])
                }
                CtrlU => {
                    self.buffer.update_position(0);
                    self.buffer.delete_after();
                    Continue(&[EraseLine, PrintPrompt])
                }
                CtrlW => {
                    if self.buffer.delete_previous_word().is_ok() {
                        ContinueWithPosition(
                            &[
                                EraseLine,
                                PrintPrompt,
                                PrintBufferToPos,
                                SaveCursor,
                                PrintBufferFromPos,
                                RestoreCursor,
                            ],
                            self.buffer.get_position(),
                        )
                    } else {
                        Continue(&[Bell])
                    }
                }
                Enter => Done(&[Newline]),
                CtrlH | Backspace => {
                    if self.buffer.backspace().is_ok() {
                        let pos = self.buffer.get_position();

                        ContinueWithPosition(
                            &[
                                MoveCursorBack,
                                SaveCursor,
                                EraseFromCursor,
                                PrintBufferFromPos,
                                RestoreCursor,
                            ],
                            pos,
                        )
                    } else {
                        Continue(&[Bell])
                    }
                }
                _ => Continue(&[Bell]),
            },
            Action::ControlSequenceIntroducer(csi) => match csi {
                CSI::CUF => self.move_cursor(CursorMove::Forward),
                CSI::CUB => self.move_cursor(CursorMove::Back),
                CSI::Unknown(_) => Continue(&[Bell]),
                CSI::CUU => Continue(&[Bell]),
                CSI::CUD => Continue(&[Bell]),
                CSI::CPR(_row, column) => {
                    self.line_width.get_or_insert(column);
                    Continue(&[])
                }
            },
            Action::EscapeSequence(_) => Continue(&[Bell]),
            Action::Ignore => Continue(&[]),
            Action::InvalidUtf8 => Continue(&[Bell]),
        }
    }
}

#[cfg(any(test, feature = "std"))]
pub mod blocking {
    use super::*;
    use crate::line_buffer::LineBuffer;
    use std::io::{Bytes, Stdin, Stdout, Write};
    use termion::raw::RawTerminal;

    pub fn readline<'a, LB: LineBuffer>(
        buffer: &'a mut LB,
        prompt: &[u8],
        stdin: &mut Bytes<Stdin>,
        stdout: &mut RawTerminal<Stdout>,
    ) -> Result<&'a [u8], ()> {
        let mut noline = Noline::new(buffer, prompt);

        let (instructions, position, _) = noline.init().args();

        for instruction in instructions {
            let bytes = noline.get_instruction(*instruction, position);

            stdout.write_all(bytes).unwrap();
        }

        stdout.flush().unwrap();

        for i in stdin {
            if let Ok(byte) = i {
                let (instructions, position, done) = noline.advance(byte).args();

                for instruction in instructions {
                    let bytes = noline.get_instruction(*instruction, position);

                    stdout.write_all(bytes).unwrap();
                }

                if let Some(status) = done {
                    if status.is_ok() {
                        return Ok(buffer.as_slice());
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
    use std::str::from_utf8;
    use std::vec::Vec;

    use crate::input::tests::AsByteVec;
    use crate::line_buffer::StaticLineBuffer;

    use super::*;

    struct MockTerminal<'a, LB: LineBuffer> {
        noline: Noline<'a, LB>,
        buffer: Vec<u8>,
        pos: Option<usize>,
        cursor: usize,
    }

    impl<'a, LB: LineBuffer> MockTerminal<'a, LB> {
        fn new(noline: Noline<'a, LB>) -> Self {
            Self {
                noline,
                buffer: Vec::new(),
                pos: None,
                cursor: 0,
            }
        }

        fn init(&mut self) {
            let (instructions, position, _) = self.noline.init().args();

            self.handle_instructions(instructions, position);
        }

        fn cursor_from_pos(&self, pos: usize) -> Option<usize> {
            if let Ok(s) = from_utf8(&self.buffer.as_slice()[0..pos]) {
                Some(s.chars().count())
            } else {
                None
            }
        }

        fn pos_from_cursor(&self, cursor_pos: usize) -> Option<usize> {
            if let Ok(s) = from_utf8(self.buffer.as_slice()) {
                if let Some((pos, _)) = s.char_indices().nth(cursor_pos) {
                    Some(pos)
                } else {
                    Some(self.buffer.len())
                }
            } else {
                None
            }
        }

        fn get_pos(&self) -> Option<usize> {
            self.pos_from_cursor(self.cursor)
        }

        fn insert_bytes(&mut self, bytes: &[u8]) {
            if let Some(pos) = self.get_pos() {
                self.pos = Some(pos);
            }

            for b in bytes {
                self.buffer.insert(self.pos.unwrap(), *b);
                self.pos = self.pos.map(|i| i + 1);
            }

            if let Some(cursor) = self.cursor_from_pos(self.pos.unwrap()) {
                self.cursor = cursor;
                self.pos = None;
            }
        }

        fn handle_instructions(&mut self, instructions: &[Instruction], position: Option<usize>) {
            let mut cursor = None;

            for instruction in instructions {
                dbg!(instruction);
                match instruction {
                    PrintPrompt | PrintBuffer | PrintBufferFromPos | PrintBufferToPos => {
                        let bytes: Vec<u8> = self
                            .noline
                            .get_instruction(*instruction, position)
                            .iter()
                            .map(|b| *b)
                            .collect();
                        dbg!(self.buffer.as_slice(), bytes.as_slice());
                        self.insert_bytes(bytes.as_slice());
                    }
                    EraseLine | ClearScreen => {
                        self.buffer.truncate(0);
                        self.cursor = 0;
                    }
                    EraseFromCursor => {
                        self.buffer
                            .truncate(self.pos_from_cursor(self.cursor).unwrap());
                    }
                    MoveCursorBack => {
                        if self.cursor > 0 {
                            self.cursor -= 1;
                        }
                    }
                    MoveCursorForward => {
                        self.cursor += 1;
                    }
                    SaveCursor => cursor = Some(self.cursor),
                    RestoreCursor => self.cursor = cursor.unwrap(),
                    Bell => (),
                    Newline => (),
                    MoveCursorToEdge => (),
                    DeviceStatusReport => (),
                }
            }
        }

        fn advance(&mut self, input: impl AsByteVec) {
            for byte in input.as_byte_vec() {
                let (instructions, position, _) = self.noline.advance(byte).args();

                self.handle_instructions(instructions, position);
            }
        }
    }

    #[test]
    fn noline() {
        const LEFT: &str = "\x1b[D";
        const RIGHT: &str = "\x1b[C";

        let prompt = "> ".as_bytes();
        let mut buffer: StaticLineBuffer<64> = StaticLineBuffer::new();
        let mut terminal = MockTerminal::new(Noline::new(&mut buffer, prompt));

        terminal.init();

        assert_eq!(terminal.buffer.as_slice(), "> ".as_bytes());
        assert_eq!(terminal.cursor, 2);
        assert_eq!(terminal.get_pos(), Some(2));

        terminal.advance("a");
        assert_eq!(terminal.buffer.as_slice(), "> a".as_bytes());
        assert_eq!(terminal.cursor, 3);
        assert_eq!(terminal.get_pos(), Some(3));

        terminal.advance("b");
        assert_eq!(terminal.buffer.as_slice(), "> ab".as_bytes());
        assert_eq!(terminal.cursor, 4);
        assert_eq!(terminal.get_pos(), Some(4));

        terminal.advance(Backspace);
        assert_eq!(terminal.buffer.as_slice(), "> a".as_bytes());
        assert_eq!(terminal.cursor, 3);
        assert_eq!(terminal.get_pos(), Some(3));

        terminal.advance("æ");

        assert_eq!(terminal.buffer.as_slice(), "> aæ".as_bytes());
        assert_eq!(terminal.cursor, 4);
        assert_eq!(terminal.get_pos(), Some(5));

        terminal.advance(LEFT);

        assert_eq!(terminal.cursor, 3);
        assert_eq!(terminal.get_pos(), Some(3));

        terminal.advance(CtrlT);

        assert_eq!(terminal.buffer.as_slice(), "> æa".as_bytes());
        assert_eq!(terminal.cursor, 3);
        assert_eq!(terminal.get_pos(), Some(4));

        terminal.advance(RIGHT);

        assert_eq!(terminal.cursor, 4);
        assert_eq!(terminal.get_pos(), Some(5));

        terminal.advance(CtrlA);

        assert_eq!(terminal.cursor, 2);
        assert_eq!(terminal.get_pos(), Some(2));

        terminal.advance(CtrlT);
        assert_eq!(terminal.buffer.as_slice(), "> æa".as_bytes());
        assert_eq!(terminal.cursor, 2);
        assert_eq!(terminal.get_pos(), Some(2));

        terminal.advance(CtrlE);

        assert_eq!(terminal.cursor, 4);
        assert_eq!(terminal.get_pos(), Some(5));

        terminal.advance(CtrlT);
        assert_eq!(terminal.buffer.as_slice(), "> æa".as_bytes());
        assert_eq!(terminal.cursor, 4);
        assert_eq!(terminal.get_pos(), Some(5));

        terminal.advance(CtrlU);
        assert_eq!(terminal.buffer.as_slice(), "> ".as_bytes());
        assert_eq!(terminal.cursor, 2);
        assert_eq!(terminal.get_pos(), Some(2));

        terminal.advance("rm -rf /");
        assert_eq!(terminal.buffer.as_slice(), "> rm -rf /".as_bytes());

        terminal.advance(vec![CtrlB, CtrlB, CtrlB, CtrlB, CtrlB]);
        terminal.advance(CtrlK);
        assert_eq!(terminal.buffer.as_slice(), "> rm ".as_bytes());

        terminal.advance("file");

        assert_eq!(terminal.buffer.as_slice(), "> rm file".as_bytes());

        terminal.advance(CtrlW);
        assert_eq!(terminal.buffer.as_slice(), "> rm ".as_bytes());
        assert_eq!(terminal.cursor, 5);
        assert_eq!(terminal.get_pos(), Some(5));

        terminal.advance(CtrlW);
        assert_eq!(terminal.buffer.as_slice(), "> ".as_bytes());
        assert_eq!(terminal.cursor, 2);
        assert_eq!(terminal.get_pos(), Some(2));
    }
}
