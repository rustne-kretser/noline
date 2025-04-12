use super::{byte_vec::ToByteVec, csi, input_builder::InputBuilder, MockTerminal};
use crate::input::ControlCharacter;
use crate::terminal::Cursor;
use core::time::Duration;
use crossbeam::channel::{unbounded, Sender};
use std::string::String;
use std::thread;
use std::thread::JoinHandle;
use std::vec::Vec;
use ControlCharacter::*;

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
