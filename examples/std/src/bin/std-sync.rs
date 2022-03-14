use noline::sync::{std::IO, Editor};
use std::fmt::Write;
use std::io;
use termion::raw::IntoRawMode;

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout().into_raw_mode().unwrap();
    let prompt = "> ";

    let mut io = IO::new(stdin, stdout);

    let mut editor = Editor::<Vec<u8>, _>::new(&mut io).unwrap();

    loop {
        if let Ok(line) = editor.readline(prompt, &mut io) {
            write!(io, "Read: '{}'\n\r", line).unwrap();
        } else {
            break;
        }
    }
}
