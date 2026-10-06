//! Line editor for synchronous IO.
//!
//! The editor takes a struct implementing the [`embedded_io::Read`] and [`embedded_io::Write`]
//! traits.
//!
//! Use the [`crate::builder::EditorBuilder`] to build an editor.
use embedded_io::{Read, ReadExactError, Write};

use crate::error::NolineError;

use crate::history::{get_history_entries, CircularSlice, History, HistoryNavigator};
use crate::line_buffer::{Buffer, LineBuffer};

use crate::core::{Line, Prompt};
use crate::output::{Event, Output};
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
    nav: HistoryNavigator<H>,
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
    /// Create an editor. Terminal initialization occurs when starting a line.
    pub fn new<IO: Read + Write>(
        buffer: LineBuffer<B>,
        history: H,
        _io: &mut IO,
    ) -> Result<Self, NolineError> {
        let terminal = Terminal::default();

        Ok(Self {
            buffer,
            terminal,
            nav: HistoryNavigator::new(history),
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
        let mut result = None;
        for item in output {
            if let Some(bytes) = item.get_bytes() {
                io.write_all(bytes)?;
            }

            result = Some(match item.event() {
                Some(Event::Submitted) => Ok(Some(())),
                Some(Event::Aborted) => Err(NolineError::Aborted),
                _ => Ok(None),
            });
        }
        if result.is_some() {
            io.flush()?;
        }
        result.unwrap_or(Ok(None))
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

    /// Borrow a line for application-driven editing. Start it before feeding input.
    pub fn line<'item, I>(&mut self, prompt: impl Into<Prompt<I>>) -> Line<'_, B, H, I>
    where
        I: Iterator<Item = &'item str> + Clone,
    {
        Line::new(prompt, &mut self.buffer, &mut self.terminal, &mut self.nav)
    }

    /// Read a line from the supplied I/O.
    pub fn readline<'a, 'item, IO, I>(
        &'a mut self,
        prompt: impl Into<Prompt<I>>,
        io: &mut IO,
    ) -> Result<&'a str, NolineError>
    where
        IO: Read + Write,
        I: Iterator<Item = &'item str> + Clone,
    {
        let mut line = self.line(prompt);
        Self::handle_output(line.start(), io)?;

        loop {
            let byte = Self::read_byte(io)?;

            if Self::handle_output(line.advance(byte)?, io)?.is_some() {
                break;
            }
        }

        Ok(self.buffer.as_str())
    }

    /// Load history from iterator
    pub fn load_history<'a>(&mut self, entries: impl Iterator<Item = &'a str>) -> usize {
        self.nav.history.load_entries(entries)
    }

    /// Get history as iterator over circular slices
    pub fn get_history(&self) -> impl Iterator<Item = CircularSlice<'_>> {
        get_history_entries(&self.nav.history)
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
        flushes: usize,
    }

    impl MockStdout {
        fn new(tx: Sender<u8>) -> Self {
            Self {
                buffer: Vec::new(),
                tx,
                flushes: 0,
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
            let buf = &buf[..buf.len().min(1)];
            self.stdout.buffer.extend(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> Result<(), Self::Error> {
            self.stdout.flushes += 1;
            for byte in self.stdout.buffer.drain(0..) {
                self.stdout.tx.send(byte).unwrap();
            }

            Ok(())
        }
    }

    impl core::fmt::Write for MockIO {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            self.write_all(s.as_bytes()).or(Err(core::fmt::Error))?;
            Ok(())
        }
    }

    #[test]
    fn flush_output_batches() {
        for end in [b'\r', 3] {
            let (tx, input) = unbounded();
            let (output, rx) = unbounded();
            let mut io = MockIO::new(MockStdin::new(input), MockStdout::new(output));
            for byte in b"\x1b[4;80R\x1b[1;3R\x1b".iter().copied().chain([end]) {
                tx.send(byte).unwrap();
            }
            drop(tx);
            let mut editor = EditorBuilder::new_unbounded().build_sync(&mut io).unwrap();
            assert!(matches!(
                (end, editor.readline("> ", &mut io)),
                (b'\r', Ok("")) | (3, Err(crate::error::NolineError::Aborted))
            ));
            assert_eq!(io.stdout.flushes, 4);
            assert!(io.stdout.buffer.is_empty());
            assert!(rx.try_iter().collect::<Vec<_>>().ends_with(b"\n\r"));
        }
    }

    #[test]
    fn invalid_geometry_recovers() {
        for report in [
            std::format!("\x1b[{};1R", usize::MAX),
            "\x1b[4;20R\x1b[5;1R".into(),
        ] {
            let (tx, rx) = unbounded();
            let (output_tx, _output_rx) = unbounded();
            let mut io = MockIO::new(MockStdin::new(rx), MockStdout::new(output_tx));
            for byte in (report + "\x1b[4;20R\x1b[1;3Rok\r").bytes() {
                tx.send(byte).unwrap();
            }
            drop(tx);
            let mut editor = EditorBuilder::new_unbounded().build_sync(&mut io).unwrap();
            assert!(matches!(
                editor.readline("> ", &mut io),
                Err(crate::error::NolineError::Aborted)
            ));
            assert_eq!(editor.readline("> ", &mut io).unwrap(), "ok");
        }
    }

    #[test]
    fn prompts_borrow_per_line() {
        let mut io = MockIO::new(
            MockStdin::new(unbounded().1),
            MockStdout::new(unbounded().0),
        );
        let mut editor = EditorBuilder::new_unbounded()
            .with_unbounded_history()
            .build_sync(&mut io)
            .unwrap();
        for text in ["first", "second"] {
            let prompt = std::format!("{text}> ");
            let mut line = editor.line(prompt.as_str());
            line.start_at(4, 80, 0).unwrap().into_vec();
            if text == "second" {
                line.advance(0x10).unwrap().into_vec();
                assert_eq!(line.as_str(), "first");
                line.advance(0x0e).unwrap().into_vec();
            }
            for byte in text.bytes().chain([b'\r']) {
                line.advance(byte).unwrap().into_iter().for_each(drop);
            }
            let input = line.into_str();
            drop(prompt);
            assert_eq!(input, text);
        }
        assert_eq!(editor.get_history().count(), 2);
    }

    #[test]
    fn simple_test() {
        let (input_tx, input_rx) = unbounded();
        let (output_tx, output_rx) = unbounded();

        let mut io = MockIO::new(MockStdin::new(input_rx), MockStdout::new(output_tx));

        let handle = thread::spawn(move || {
            let mut editor = EditorBuilder::new_unbounded().build_sync(&mut io).unwrap();

            let result = {
                let prompt = std::string::String::from("> ");
                editor.readline(prompt.as_str(), &mut io)
            };
            if let Ok(s) = result {
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

        for &b in b"\x1b[6n" {
            let received = output_rx
                .recv_timeout(::core::time::Duration::from_millis(1000))
                .unwrap();
            println!("Received {:x}, expected: {:x}", received, b);
            assert_eq!(received, b);
        }

        for &b in b"\x1b[1;1R" {
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
