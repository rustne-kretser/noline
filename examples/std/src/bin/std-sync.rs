use noline::sync::with_std::Editor;
use std::io::{self, Write};
use termion::raw::IntoRawMode;

fn main() {
    let mut stdin = io::stdin();
    let mut stdout = io::stdout().into_raw_mode().unwrap();
    let prompt = "> ";

    let mut editor = Editor::<Vec<u8>>::new(prompt, &mut stdin, &mut stdout).unwrap();

    loop {
        if let Ok(line) = editor.readline(&mut stdin, &mut stdout) {
            write!(stdout, "Read: '{}'\n\r", line).unwrap();
        } else {
            break;
        }
    }
}
