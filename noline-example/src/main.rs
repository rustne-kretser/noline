use noline::sync::with_std::readline;
use std::io::{self, Write};
use termion::raw::IntoRawMode;

use noline::line_buffer::AllocLineBuffer;

fn main() {
    let mut stdin = io::stdin();
    let mut stdout = io::stdout().into_raw_mode().unwrap();
    let prompt = "> ";

    loop {
        let mut buffer = AllocLineBuffer::new();

        if let Ok(line) = readline(&mut buffer, prompt, &mut stdin, &mut stdout) {
            write!(stdout, "Read: '{}'\n\r", line).unwrap();
        } else {
            break;
        }
    }
}
