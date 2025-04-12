use crate::input::{Action, ControlCharacter, Parser, CSI};
use crate::terminal::Cursor;
use crossbeam::channel::{unbounded, Receiver, Sender};
use std::string::String;
use std::thread;
use std::thread::JoinHandle;
use std::vec::Vec;

pub struct MockTerminal {
    parser: Parser,
    screen: Vec<Vec<char>>,
    pub cursor: Cursor,
    pub rows: usize,
    pub columns: usize,
    saved_cursor: Option<Cursor>,
    pub bell: bool,
    pub terminal_tx: Option<Sender<u8>>,
    pub terminal_rx: Receiver<u8>,
    pub keyboard_tx: Sender<u8>,
    pub keyboard_rx: Receiver<u8>,
}

impl MockTerminal {
    pub fn new(rows: usize, columns: usize, origin: Cursor) -> Self {
        let (terminal_tx, terminal_rx) = unbounded();
        let (keyboard_tx, keyboard_rx) = unbounded();

        Self {
            parser: Parser::new(),
            screen: vec![vec!['\0'; columns]; rows],
            cursor: origin,
            rows,
            columns,
            saved_cursor: None,
            bell: false,
            terminal_tx: Some(terminal_tx),
            terminal_rx,
            keyboard_tx,
            keyboard_rx,
        }
    }

    pub fn current_line(&mut self) -> &mut Vec<char> {
        let cursor = self.get_cursor();

        &mut self.screen[cursor.row]
    }

    pub fn screen_as_string(&self) -> String {
        self.screen
            .iter()
            .map(|v| v.iter().take_while(|&&c| c != '\0').collect::<String>())
            .filter(|s| !s.is_empty())
            .collect::<Vec<String>>()
            .join("\n")
    }

    pub fn current_line_as_string(&self) -> String {
        self.screen[self.cursor.row]
            .iter()
            .take_while(|&&c| c != '\0')
            .collect()
    }

    fn move_column(&mut self, steps: isize) {
        self.cursor.column =
            0.max((self.cursor.column as isize + steps).min(self.columns as isize - 1)) as usize;
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

    pub fn advance(&mut self, byte: u8) -> Option<Vec<u8>> {
        let mock_term_action = self.parser.advance(byte);

        dbg!(mock_term_action);
        match mock_term_action {
            Action::Ignore => (),
            Action::Print(c) => {
                let pos = self.cursor.column;
                let line = self.current_line();

                line[pos] = c.as_char();
                self.move_column(1);
            }
            Action::ControlSequenceIntroducer(csi) => match csi {
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

                    for row in cursor.row..self.rows {
                        let start = if row == cursor.row { cursor.column } else { 0 };
                        for column in (start)..self.columns {
                            self.screen[row][column] = '\0';
                        }
                    }
                }
                CSI::DSR => {
                    return Some(
                        format!("\x1b[{};{}R", self.cursor.row + 1, self.cursor.column + 1)
                            .bytes()
                            .collect::<Vec<u8>>(),
                    );
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
                CSI::Invalid => unimplemented!(),
            },
            Action::InvalidUtf8 => unreachable!(),
            Action::ControlCharacter(ctrl) => {
                dbg!(ctrl);

                match ctrl {
                    ControlCharacter::CarriageReturn => self.cursor.column = 0,
                    ControlCharacter::LineFeed => {
                        if self.cursor.row + 1 == self.rows {
                            self.scroll_up(1);
                        } else {
                            self.cursor.row += 1;
                        }
                    }
                    ControlCharacter::CtrlG => self.bell = true,
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

    pub fn get_cursor(&self) -> Cursor {
        self.cursor
    }

    pub fn listen(&mut self) {
        while let Ok(b_in) = self.terminal_rx.recv() {
            if let Some(output) = self.advance(b_in) {
                for b_out in output {
                    self.keyboard_tx.send(b_out).unwrap();
                }
            }
        }
    }

    pub fn start_thread(mut self) -> JoinHandle<Self> {
        thread::spawn(move || {
            self.listen();
            self
        })
    }

    pub fn take_io(&mut self) -> (Option<Sender<u8>>, Receiver<u8>) {
        (self.terminal_tx.take(), self.keyboard_rx.clone())
    }
}
