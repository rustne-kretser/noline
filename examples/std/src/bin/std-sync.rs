use noline::builder::EditorBuilder;
use std::io;
use termion::raw::IntoRawMode;

use embedded_io::{ErrorType, Read as EmbRead, Write as EmbWrite};
use std::io::{Read, Stdin, Stdout, Write};

pub struct IOWrapper {
    stdin: Stdin,
    stdout: Stdout,
}

impl IOWrapper {
    pub fn new() -> Self {
        Self {
            stdin: std::io::stdin(),
            stdout: std::io::stdout(),
        }
    }
}

impl Default for IOWrapper {
    fn default() -> Self {
        Self::new()
    }
}

impl ErrorType for IOWrapper {
    type Error = embedded_io::ErrorKind;
}

impl EmbRead for IOWrapper {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        Ok(self.stdin.read(buf).map_err(|e| e.kind())?)
    }
}

impl EmbWrite for IOWrapper {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        Ok(self.stdout.write(buf).map_err(|e| e.kind())?)
    }
    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(self.stdout.flush().map_err(|e| e.kind())?)
    }
}

fn main() {
    let _stdout = io::stdout().into_raw_mode().unwrap();
    let prompt = "> ";

    let mut io = IOWrapper::new();

    let mut editor = EditorBuilder::new_unbounded()
        .with_alloc_history(100)
        .build_sync(&mut io)
        .unwrap();

    while let Ok(line) = editor.readline(prompt, &mut io) {
        writeln!(io, "Read: '{}'", line).unwrap();
    }
}
