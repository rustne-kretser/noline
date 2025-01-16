use core::time::Duration;
use std::string::String;
use std::thread;
use std::thread::JoinHandle;
use std::vec::Vec;

use crossbeam::channel::{unbounded, Receiver, Sender};

use crate::input::{Action, ControlCharacter, Parser, CSI};
use crate::terminal::Cursor;

use ControlCharacter::*;

pub mod csi {
    pub const UP: &str = "\x1b[A";
    pub const DOWN: &str = "\x1b[B";
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

impl ToByteVec for &str {
    fn to_byte_vec(self) -> Vec<u8> {
        self.bytes().collect()
    }
}

impl ToByteVec for ControlCharacter {
    fn to_byte_vec(self) -> Vec<u8> {
        [self.into()].into_iter().collect()
    }
}

impl ToByteVec for Vec<ControlCharacter> {
    fn to_byte_vec(self) -> Vec<u8> {
        self.into_iter().map(|c| c.into()).collect()
    }
}

impl<const N: usize> ToByteVec for [ControlCharacter; N] {
    fn to_byte_vec(self) -> Vec<u8> {
        self.into_iter().map(|c| c.into()).collect()
    }
}

impl ToByteVec for Vec<&str> {
    fn to_byte_vec(self) -> Vec<u8> {
        self.into_iter()
            .flat_map(|s| s.as_bytes().iter().copied())
            .collect()
    }
}

impl<const N: usize> ToByteVec for [&str; N] {
    fn to_byte_vec(self) -> Vec<u8> {
        self.into_iter()
            .flat_map(|s| s.as_bytes().iter().copied())
            .collect()
    }
}

pub trait ToByteVec {
    fn to_byte_vec(self) -> Vec<u8>;
}

#[derive(Debug)]
pub struct TestCase {
    pub input: Vec<Vec<u8>>,
    pub output: Vec<String>,
}

impl TestCase {
    pub fn new(
        input: impl IntoIterator<Item = impl ToByteVec>,
        output: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            input: input.into_iter().map(|item| item.to_byte_vec()).collect(),
            output: output.into_iter().map(|s| s.into()).collect(),
        }
    }

    pub fn screen_as_string(&self, prompt: &str, columns: usize) -> String {
        let mut screen = Vec::new();
        let mut line = Vec::new();

        line.extend(prompt.chars());

        for s in &self.output {
            for c in s.chars() {
                line.push(c);

                if line.len() >= columns {
                    screen.extend(line.drain(0..));
                    screen.push('\n');
                }
            }

            if !line.is_empty() {
                screen.extend(line.drain(0..));
                screen.push('\n');
                screen.extend(prompt.chars());
            }
        }

        screen.into_iter().collect()
    }
}

struct InputBuilder {
    items: Vec<u8>,
}

impl InputBuilder {
    fn new() -> Self {
        Self { items: Vec::new() }
    }

    fn add(&mut self, input: impl ToByteVec) {
        self.items.extend(input.to_byte_vec().iter());
    }
}

impl ToByteVec for InputBuilder {
    fn to_byte_vec(self) -> Vec<u8> {
        self.items
    }
}

pub fn test_cases() -> Vec<TestCase> {
    vec![
        TestCase::new(["Hello, World!"], ["Hello, World!"]),
        {
            let mut input = InputBuilder::new();

            input.add("abc");
            input.add(csi::LEFT);
            input.add(CtrlD);
            input.add("de");

            TestCase::new([input], ["abde"])
        },
        TestCase::new(["abc", "def"], ["abc", "def"]),
    ]
}

pub fn test_editor_with_case<IO: Send + 'static>(
    case: TestCase,
    prompt: &str,
    get_io: impl FnOnce(&mut MockTerminal) -> IO,
    spawn_thread: impl FnOnce(IO, Sender<String>) -> JoinHandle<()>,
) {
    let (rows, columns) = (20, 80);

    let (string_tx, string_rx) = unbounded();

    let mut term = MockTerminal::new(rows, columns, Cursor::new(0, 0));

    let keyboard_tx = term.keyboard_tx.clone();

    let io = get_io(&mut term);

    let term = term.start_thread();
    let handle = spawn_thread(io, string_tx);

    let output: Vec<String> = case
        .input
        .iter()
        .map(|seq| {
            // To avoid race with prompt reset, we need to wait a
            // little. This is not ideal, but will do for now.
            thread::sleep(core::time::Duration::from_millis(100));

            for &b in seq {
                keyboard_tx.send(b).unwrap();
            }

            keyboard_tx.send(0xd).unwrap();

            string_rx.recv().unwrap()
        })
        .collect();

    // Added delay to prevent race with terminal reset
    std::thread::sleep(Duration::from_millis(100));

    keyboard_tx.send(0x3).unwrap();

    drop(keyboard_tx);
    let term = term.join().unwrap();

    handle.join().unwrap();

    assert_eq!(output.len(), case.output.len());

    for (seen, expected) in output.iter().zip(case.output.iter()) {
        assert_eq!(seen, expected);
    }

    assert_eq!(
        term.screen_as_string(),
        case.screen_as_string(prompt, columns)
    );
}
