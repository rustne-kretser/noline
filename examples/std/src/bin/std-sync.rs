use noline::builder::EditorBuilder;
use std::io;
use termion::raw::IntoRawMode;

use embedded_io::Write;

#[path = "../io_wrapper.rs"]
mod io_wrapper;
use io_wrapper::IOWrapper;

fn main() {
    let _stdout = io::stdout().into_raw_mode().unwrap();
    let prompt = "> ";

    let mut io = IOWrapper::new();

    let mut editor = EditorBuilder::new_unbounded()
        .with_unbounded_history()
        .build_sync(&mut io)
        .unwrap();

    while let Ok(line) = editor.readline(prompt, &mut io) {
        writeln!(io, "Read: '{}'", line).unwrap();
    }
}
