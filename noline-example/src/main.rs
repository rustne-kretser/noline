use noline::blocking::readline;
use std::io::Read;
use std::io::{self, Write};
use termion::raw::IntoRawMode;

use noline::line_buffer::AllocLineBuffer;

fn main() {
    let mut stdin = io::stdin().bytes();
    let mut stdout = io::stdout().into_raw_mode().unwrap();
    let prompt = "> ".as_bytes();

    loop {
        let mut buffer = AllocLineBuffer::new();

        if let Ok(bytes) = readline(&mut buffer, prompt, &mut stdin, &mut stdout) {
            if let Ok(line) = std::str::from_utf8(bytes) {
                write!(stdout, "Read: '{}'\n\r", line).unwrap();
            } else {
                write!(stdout, "Read (invalid UTF-8): '{:?}'\n\r", bytes).unwrap();
            }
        } else {
            break;
        }
    }
}
