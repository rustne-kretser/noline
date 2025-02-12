//! Line editor for synchronous IO.
//!
//! The editor takes a struct implementing the [`embedded_io::Read`] and [`embedded_io::Write`]
//! traits.
//!
//! Use the [`crate::builder::EditorBuilder`] to build an editor.
use embedded_io::{Read, ReadExactError, Write};

use crate::error::NolineError;

use crate::history::{get_history_entries, CircularSlice, History};
use crate::line_buffer::{Buffer, LineBuffer};

use crate::core::{Line, Prompt};
use crate::output::{Output, OutputItem};
use crate::terminal::Terminal;

/// Line editor for synchronous IO
///
/// It is recommended to use [`crate::builder::EditorBuilder`] to build an Editor.
pub struct Editor<B, H>
where
    B: Buffer,
    H: History,
{
    buffer: LineBuffer<B>,
    terminal: Terminal,
    history: H,
}

impl<E> From<E> for NolineError
where
    E: embedded_io::Error,
{
    fn from(value: E) -> Self {
        NolineError::IoError(value.kind())
    }
}

impl<B, H> Editor<B, H>
where
    B: Buffer,
    H: History,
{
    /// Create and initialize line editor
    pub fn new<IO: Read + Write>(
        buffer: LineBuffer<B>,
        history: H,
        _io: &mut IO,
    ) -> Result<Self, NolineError> {
        let terminal = Terminal::default();

        Ok(Self {
            buffer,
            terminal,
            history,
        })
    }

    fn handle_output<'a, 'item, IO, I>(
        output: Output<'a, B, I>,
        io: &mut IO,
    ) -> Result<Option<()>, NolineError>
    where
        IO: Read + Write,
        I: Iterator<Item = &'item str> + Clone,
    {
        for item in output {
            if let Some(bytes) = item.get_bytes() {
                io.write(bytes)?;
            }

            io.flush()?;

            match item {
                OutputItem::EndOfString => return Ok(Some(())),
                OutputItem::Abort => return Err(NolineError::Aborted),
                _ => (),
            }
        }

        Ok(None)
    }

    fn read_byte<IO>(io: &mut IO) -> Result<u8, NolineError>
    where
        IO: Read + Write,
    {
        let mut buf = [0x8; 1];

        match io.read_exact(&mut buf) {
            Ok(_) => Ok(buf[0]),
            Err(err) => match err {
                ReadExactError::UnexpectedEof => Err(NolineError::Aborted),
                ReadExactError::Other(err) => Err(err)?,
            },
        }
    }

    /// Read line from `stdin`
    pub fn readline<'a, 'item, IO, I>(
        &'a mut self,
        prompt: impl Into<Prompt<I>>,
        io: &mut IO,
    ) -> Result<&str, NolineError>
    where
        IO: Read + Write,
        I: Iterator<Item = &'item str> + Clone,
    {
        let mut line = Line::new(
            prompt,
            &mut self.buffer,
            &mut self.terminal,
            &mut self.history,
        );

        let mut reset = line.reset();

        Self::handle_output(reset.start(), io)?;

        loop {
            let byte = Self::read_byte(io)?;

            if let Some(output) = reset.advance(byte) {
                Self::handle_output(output, io)?;
            } else {
                break;
            }
        }

        loop {
            let byte = Self::read_byte(io)?;

            if Self::handle_output(line.advance(byte), io)?.is_some() {
                break;
            }
        }

        Ok(self.buffer.as_str())
    }

    /// Load history from iterator
    pub fn load_history<'a>(&mut self, entries: impl Iterator<Item = &'a str>) -> usize {
        self.history.load_entries(entries)
    }

    /// Get history as iterator over circular slices
    pub fn get_history(&self) -> impl Iterator<Item = CircularSlice<'_>> {
        get_history_entries(&self.history)
    }
}

#[cfg(test)]
pub mod tests {
    //! IO implementation for `std`. Requires feature `std`.

    use std::string::ToString;
    use std::{thread, vec::Vec};

    use crossbeam::channel::{unbounded, Receiver, Sender};
    use embedded_io::{Read, Write};

    use crate::builder::EditorBuilder;
    use crate::testlib::{test_cases, test_editor_with_case, MockTerminal};

    struct MockStdout {
        buffer: Vec<u8>,
        tx: Sender<u8>,
    }

    impl MockStdout {
        fn new(tx: Sender<u8>) -> Self {
            Self {
                buffer: Vec::new(),
                tx,
            }
        }
    }

    struct MockStdin {
        rx: Receiver<u8>,
    }

    impl MockStdin {
        fn new(rx: Receiver<u8>) -> Self {
            Self { rx }
        }
    }

    struct MockIO {
        stdin: MockStdin,
        stdout: MockStdout,
    }

    impl MockIO {
        fn new(stdin: MockStdin, stdout: MockStdout) -> Self {
            Self { stdout, stdin }
        }

        fn from_terminal(terminal: &mut MockTerminal) -> Self {
            let (tx, rx) = terminal.take_io();

            Self::new(MockStdin::new(rx), MockStdout::new(tx.unwrap()))
        }

        fn get_pipes(self) -> (MockStdin, MockStdout) {
            (self.stdin, self.stdout)
        }
    }

    impl embedded_io::ErrorType for MockIO {
        type Error = embedded_io::ErrorKind;
    }

    impl embedded_io::Read for MockIO {
        fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
            for place in &mut *buf {
                match self.stdin.rx.recv() {
                    Ok(byte) => *place = byte,
                    // This should never happen as the error type is Infalliable
                    Err(_) => return Err(Self::Error::Other),
                }
            }

            Ok(buf.len())
        }
    }

    impl embedded_io::Write for MockIO {
        fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
            self.stdout.buffer.extend(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> Result<(), Self::Error> {
            for byte in self.stdout.buffer.drain(0..) {
                self.stdout.tx.send(byte).unwrap();
            }

            Ok(())
        }
    }

    impl core::fmt::Write for MockIO {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            self.write(s.as_bytes()).or(Err(core::fmt::Error))?;
            Ok(())
        }
    }

    #[test]
    fn simple_test() {
        let (input_tx, input_rx) = unbounded();
        let (output_tx, output_rx) = unbounded();

        let mut io = MockIO::new(MockStdin::new(input_rx), MockStdout::new(output_tx));

        let handle = thread::spawn(move || {
            let mut editor = EditorBuilder::new_unbounded().build_sync(&mut io).unwrap();

            if let Ok(s) = editor.readline("> ", &mut io) {
                Some(s.to_string())
            } else {
                None
            }
        });

        for &b in b"\x1b7\x1b[999;999H\x1b[6n\x1b8" {
            let received = output_rx
                .recv_timeout(::core::time::Duration::from_millis(1000))
                .unwrap();
            println!("Received {:x}, expected: {:x}", received, b);
            assert_eq!(received, b);
        }

        for &b in b"\x1b[20;80R" {
            input_tx.send(b).unwrap();
        }

        for &b in b"\r\x1b[J> \x1b[6n" {
            let received = output_rx
                .recv_timeout(::core::time::Duration::from_millis(1000))
                .unwrap();
            println!("Received {:x}, expected: {:x}", received, b);
            assert_eq!(received, b);
        }

        for &b in b"\x1b[1;3R" {
            input_tx.send(b).unwrap();
        }

        for &b in "abc\r".as_bytes() {
            input_tx.send(b).unwrap();
        }

        assert_eq!(handle.join().unwrap(), Some("abc".to_string()));
    }

    #[test]
    fn mock_stdin() {
        let (tx, rx) = unbounded();

        let mut io = MockIO::new(MockStdin::new(rx), MockStdout::new(tx));
        for i in 0u8..10 {
            io.write(&[i]).unwrap();
        }

        io.flush().unwrap();

        let mut buf = [0];
        for i in 0..10 {
            io.read(&mut buf).unwrap();

            assert_eq!(buf[0], i);
        }
    }

    #[test]
    fn editor() {
        let prompt = "> ";

        for case in test_cases() {
            test_editor_with_case(
                case,
                prompt,
                |term| MockIO::from_terminal(term).get_pipes(),
                |(stdin, stdout), string_tx| {
                    thread::spawn(move || {
                        let mut io = MockIO::new(stdin, stdout);
                        let mut editor = EditorBuilder::new_unbounded()
                            .with_unbounded_history()
                            .build_sync(&mut io)
                            .unwrap();

                        while let Ok(s) = editor.readline(prompt, &mut io) {
                            string_tx.send(s.to_string()).unwrap();
                        }
                    })
                },
            )
        }
    }
}
