use embedded_io::{Read, Write};
use noline::{
    builder::EditorBuilder,
    editor::{Event, OutputItem},
};
use std::io;
use termion::raw::IntoRawMode;

#[path = "../io_wrapper.rs"]
mod io_wrapper;
use io_wrapper::IOWrapper;

const HELP: &[u8] = b"Tab: complete 'hello' or show help. Ctrl-C: exit.\r\n";

fn render<'a>(
    io: &mut IOWrapper,
    output: impl IntoIterator<Item = OutputItem<'a>>,
) -> Option<Event> {
    let mut event = None;
    for item in output {
        if let Some(bytes) = item.get_bytes() {
            io.write_all(bytes).unwrap();
        }
        if let Some(outcome) = item.event() {
            event = Some(outcome);
        }
    }
    io.flush().unwrap();
    event
}

fn main() {
    let _stdout = io::stdout().into_raw_mode().unwrap();
    let prompt = "> ";
    let mut io = IOWrapper::new();
    let mut editor = EditorBuilder::new_unbounded()
        .with_unbounded_history()
        .build_sync(&mut io)
        .unwrap();

    io.write_all(HELP).unwrap();
    loop {
        let mut line = editor.line(prompt);
        render(&mut io, line.start());
        loop {
            let mut byte = [0];
            if io.read(&mut byte).unwrap() == 0 {
                return;
            }
            let output = match (byte[0], line.cursor()) {
                (b'\t', Some(cursor)) => {
                    let prefix = &line.as_str()[..cursor];
                    if cursor == line.as_str().len()
                        && prefix != "hello "
                        && "hello ".starts_with(prefix)
                    {
                        line.replace(0..cursor, "hello ").unwrap()
                    } else {
                        // Consume Tab before preserving the parser across the handoff.
                        render(&mut io, line.advance(byte[0]).unwrap());
                        render(&mut io, line.suspend().unwrap());
                        io.write_all(HELP).unwrap();
                        line.resume().unwrap()
                    }
                }
                _ => line.advance(byte[0]).unwrap(),
            };
            match render(&mut io, output) {
                Some(Event::Submitted) => break,
                Some(Event::Aborted) => return,
                None => (),
            }
        }
        write!(io, "Read: '{}'\r\n", line.into_str()).unwrap();
    }
}
