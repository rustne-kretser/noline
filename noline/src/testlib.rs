use std::string::String;
use std::thread;
use std::thread::JoinHandle;
use std::vec::Vec;

use crossbeam::channel::{unbounded, Receiver, Sender};

use crate::input::{Action, ControlCharacter, Parser, CSI};
use crate::terminal::Cursor;

use ControlCharacter::*;

pub mod csi {
    pub const LEFT: &str = "\x1b[D";
    pub const RIGHT: &str = "\x1b[C";
    pub const HOME: &str = "\x1b[1~";
    pub const DELETE: &str = "\x1b[3~";
    pub const END: &str = "\x1b[4~";
}

pub struct MockTerminal {
    parser: Parser,
    screen: Vec<Vec<char>>,
    pub cursor: Cursor,
    rows: usize,
    columns: usize,
    saved_cursor: Option<Cursor>,
    pub bell: bool,
    pub terminal_tx: Sender<Option<u8>>,
    pub terminal_rx: Receiver<Option<u8>>,
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
            terminal_tx,
            terminal_rx,
            keyboard_tx,
            keyboard_rx,
        }
    }

    pub fn current_line(&mut self) -> &mut Vec<char> {
        let cursor = self.get_cursor();

        &mut self.screen[cursor.row as usize]
    }

    pub fn screen_as_string(&self) -> String {
        self.screen
            .iter()
            .map(|v| v.iter().take_while(|&&c| c != '\0').collect::<String>())
            .filter(|s| s.len() > 0)
            .collect::<Vec<String>>()
            .join("\n")
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
                    return Some(
                        format!("\x1b[{};{}R", self.cursor.row + 1, self.cursor.column + 1,)
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
        loop {
            if let Ok(b_in) = self.terminal_rx.recv() {
                if let Some(b_in) = b_in {
                    if let Some(output) = self.advance(b_in) {
                        for b_out in output {
                            self.keyboard_tx.send(b_out).unwrap();
                        }
                    }
                } else {
                    break;
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
}

impl AsByteVec for &str {
    fn as_byte_vec(self) -> Vec<u8> {
        self.bytes().into_iter().collect()
    }
}

impl AsByteVec for ControlCharacter {
    fn as_byte_vec(self) -> Vec<u8> {
        [self.into()].into_iter().collect()
    }
}

impl AsByteVec for Vec<ControlCharacter> {
    fn as_byte_vec(self) -> Vec<u8> {
        self.into_iter().map(|c| c.into()).collect()
    }
}

impl<const N: usize> AsByteVec for [ControlCharacter; N] {
    fn as_byte_vec(self) -> Vec<u8> {
        self.into_iter().map(|c| c.into()).collect()
    }
}

impl AsByteVec for Vec<&str> {
    fn as_byte_vec(self) -> Vec<u8> {
        self.into_iter()
            .map(|s| s.as_bytes().into_iter())
            .flatten()
            .map(|&b| b)
            .collect()
    }
}

impl<const N: usize> AsByteVec for [&str; N] {
    fn as_byte_vec(self) -> Vec<u8> {
        self.into_iter()
            .map(|s| s.as_bytes().into_iter())
            .flatten()
            .map(|&b| b)
            .collect()
    }
}

pub trait AsByteVec {
    fn as_byte_vec(self) -> Vec<u8>;
}

pub struct TestCase {
    pub input: Vec<u8>,
    pub output: Vec<u8>,
}

impl TestCase {
    pub fn new(input: impl AsByteVec, output: impl AsByteVec) -> Self {
        Self {
            input: input.as_byte_vec(),
            output: output.as_byte_vec(),
        }
    }

    pub fn output_as_string(&self) -> String {
        String::from_utf8(self.output.clone()).unwrap()
    }

    pub fn screen_as_string(&self, prompt: &str, columns: usize) -> String {
        prompt
            .chars()
            .chain(self.output_as_string().chars())
            .fold(Vec::new(), |mut s, c| {
                s.push(c);

                if s.len() % columns == 0 {
                    s.push('\n');
                }
                s
            })
            .iter()
            .collect()
    }
}

struct InputBuilder {
    items: Vec<u8>,
}

impl InputBuilder {
    fn new() -> Self {
        Self { items: Vec::new() }
    }

    fn add(&mut self, input: impl AsByteVec) {
        self.items.extend(input.as_byte_vec().iter());
    }
}

impl AsByteVec for InputBuilder {
    fn as_byte_vec(self) -> Vec<u8> {
        self.items
    }
}

pub fn test_cases() -> Vec<TestCase> {
    vec![
        {
            let s = "Hello, World!";
            TestCase::new(s, s)
        },
        {
            let mut input = InputBuilder::new();

            input.add("abc");
            input.add(csi::LEFT);
            input.add(CtrlD);
            input.add("de");

            TestCase::new(input, "abde")
        },
    ]
}

pub fn test_editor_with_case<IO: Send + 'static>(
    case: TestCase,
    prompt: &str,
    get_io: impl FnOnce(&MockTerminal) -> IO,
    spawn_thread: impl FnOnce(IO) -> JoinHandle<Option<String>>,
) {
    let (rows, columns) = (20, 80);

    let term = MockTerminal::new(rows, columns, Cursor::new(0, 0));

    let keyboard_tx = term.keyboard_tx.clone();

    let io = get_io(&term);

    let term = term.start_thread();
    let handle = spawn_thread(io);

    for &b in case.input.iter() {
        keyboard_tx.send(b).unwrap();
    }

    keyboard_tx.send(0xd).unwrap();

    let term = term.join().unwrap();

    assert_eq!(handle.join().unwrap(), Some(case.output_as_string()));

    assert_eq!(
        term.screen_as_string(),
        case.screen_as_string(prompt, columns)
    );
}
