use noline::{
    builder::EditorBuilder,
    sync_io::std_sync::{StdinWrapper, StdoutWrapper},
    sync_io::IO,
};
use std::fmt::Write;
use std::io;
use termion::raw::IntoRawMode;

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout().into_raw_mode().unwrap();
    let prompt = "> ";

    let mut io = IO::<StdinWrapper, StdoutWrapper>::default();

    let mut editor = EditorBuilder::new_unbounded()
        .with_unbounded_history()
        .build_sync(&mut io)
        .unwrap();

    loop {
        if let Ok(line) = editor.readline(prompt, &mut io) {
            write!(io, "Read: '{}'\n\r", line).unwrap();
        } else {
            break;
        }
    }
}
