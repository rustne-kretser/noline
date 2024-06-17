//! Line editor for synchronous IO.
//!
//! The editor takes a struct implementing the [`Read`] and [`Write`]
//! traits. There are ready made implementations in [`std::IO`] and [`embedded::IO`].
//!
//! Use the [`crate::builder::EditorBuilder`] to build an editor.
use embedded_io::{Read, Write};

use crate::error::NolineError;

use crate::history::{get_history_entries, CircularSlice, History};
use crate::line_buffer::{Buffer, LineBuffer};

use crate::core::{Initializer, InitializerResult, Line};
use crate::output::{Output, OutputItem};
use crate::sync_io::IO;
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

impl<B, H> Editor<B, H>
where
    B: Buffer,
    H: History,
{
    /// Create and initialize line editor
    pub fn new<RW: embedded_io::Read + embedded_io::Write>(
        io: &mut IO<RW>,
    ) -> Result<Self, NolineError> {
        let mut initializer = Initializer::new();

        io.write(Initializer::init())?;

        io.flush()?;

        let terminal = loop {
            let mut buf = [0u8; 1];

            let len = io.read(&mut buf)?;
            if len == 1 {
                match initializer.advance(buf[0]) {
                    InitializerResult::Continue => (),
                    InitializerResult::Item(terminal) => break terminal,
                    InitializerResult::InvalidInput => {
                        return Err(NolineError::ParserError);
                    }
                }
            }
            if len == 0 {
                return Err(NolineError::Aborted);
            }
        };

        Ok(Self {
            buffer: LineBuffer::new(),
            terminal,
            history: H::default(),
        })
    }

    fn handle_output<'b, RW: embedded_io::Read + embedded_io::Write>(
        output: Output<'b, B>,
        io: &mut IO<RW>,
    ) -> Result<Option<()>, NolineError> {
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

    /// Read line from `stdin`
    pub fn readline<'b, RW: embedded_io::Read + embedded_io::Write>(
        &'b mut self,
        prompt: &'b str,
        io: &mut IO<RW>,
    ) -> Result<&'b str, NolineError> {
        let mut line = Line::new(
            prompt,
            &mut self.buffer,
            &mut self.terminal,
            &mut self.history,
        );
        Self::handle_output(line.reset(), io)?;

        loop {
            let mut buf = [0x8; 1];
            let len = io.read(&mut buf)?;
            if len == 1 {
                if Self::handle_output(line.advance(buf[0]), io)?.is_some() {
                    break;
                }
            }
        }

        Ok(self.buffer.as_str())
    }

    /// Load history from iterator
    pub fn load_history<'a>(&mut self, entries: impl Iterator<Item = &'a str>) -> usize {
        self.history.load_entries(entries)
    }

    /// Get history as iterator over circular slices
    pub fn get_history<'a>(&'a self) -> impl Iterator<Item = CircularSlice<'a>> {
        get_history_entries(&self.history)
    }
}

#[cfg(test)]
mod tests {
    use crate::builder::EditorBuilder;

    use super::*;
    use crossbeam::channel::{unbounded, Receiver, Sender};
    use std::string::ToString;
    use std::{thread, vec::Vec};

    struct IO {
        input: Receiver<u8>,
        buffer: Vec<u8>,
        output: Sender<u8>,
    }

    impl IO {
        fn new(input: Receiver<u8>, output: Sender<u8>) -> Self {
            Self {
                input,
                buffer: Vec::new(),
                output,
            }
        }
    }

    impl Write for IO {
        type Error = ();

        fn write(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
            dbg!(buf);
            self.buffer.extend(buf.iter());

            Ok(())
        }

        fn flush(&mut self) -> Result<(), Self::Error> {
            for b in self.buffer.drain(0..) {
                self.output.send(b).unwrap();
            }

            Ok(())
        }
    }

    impl Read for IO {
        type Error = ();

        fn read(&mut self) -> Result<u8, Self::Error> {
            Ok(self.input.recv().or_else(|_| Err(()))?)
        }
    }

    #[test]
    fn editor() {
        let (input_tx, input_rx) = unbounded();
        let (output_tx, output_rx) = unbounded();

        let mut io = IO::new(input_rx, output_tx);

        let handle = thread::spawn(move || {
            let mut editor = EditorBuilder::new_unbounded().build_sync(&mut io).unwrap();

            if let Ok(s) = editor.readline("> ", &mut io) {
                Some(s.to_string())
            } else {
                None
            }
        });

        for &b in Initializer::init() {
            dbg!(b);
            assert_eq!(
                output_rx.recv_timeout(::core::time::Duration::from_millis(1000)),
                Ok(b)
            );
        }

        for &b in "\x1b[1;1R\x1b[20;80R".as_bytes() {
            input_tx.send(b).unwrap();
        }

        for &b in "\r\x1b[J> \x1b[6n".as_bytes() {
            dbg!(b);
            assert_eq!(
                output_rx.recv_timeout(::core::time::Duration::from_millis(1000)),
                Ok(b)
            );
        }

        for &b in "abc\r".as_bytes() {
            input_tx.send(b).unwrap();
        }

        assert_eq!(handle.join().unwrap(), Some("abc".to_string()));
    }
}

#[cfg(any(test, feature = "std"))]
pub mod std {
    //! IO implementation for `std`. Requires feature `std`.

    #[cfg(test)]
    mod tests {
        use std::string::ToString;
        use std::{thread, vec::Vec};

        use crossbeam::channel::{unbounded, Receiver, Sender};

        use crate::builder::EditorBuilder;
        use crate::testlib::{test_cases, test_editor_with_case, MockTerminal};
        use std::io::Read as IoRead;

        use super::*;

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

        impl io::Read for MockStdin {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                for i in 0..(buf.len()) {
                    match self.rx.recv() {
                        Ok(byte) => buf[i] = byte,
                        Err(_) => return Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe)),
                    }
                }

                Ok(buf.len())
            }
        }

        impl io::Write for MockStdout {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.buffer.extend(buf);
                Ok(buf.len())
            }

            fn flush(&mut self) -> std::io::Result<()> {
                for byte in self.buffer.drain(0..) {
                    self.tx.send(byte).unwrap();
                }

                Ok(())
            }
        }

        #[test]
        fn mock_stdin() {
            let (tx, rx) = unbounded();

            let mut stdin = MockStdin::new(rx);

            for i in 0..10 {
                tx.send(i).unwrap();
            }

            for i in 0..10 {
                let mut buf = [0];
                stdin.read_exact(&mut buf).unwrap();

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
                            let mut io = IO::new(stdin, stdout);
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
}

#[cfg(any(test, feature = "embedded"))]
pub mod embedded {
    //! IO implementation using traits from [`embedded_io`]. Requires feature `embedded`.

    //    use core::fmt;

    //    use embedded_io;
    //use nb::block;

    //use super::*;
    //use crate::sync_io::IO;

    /// IO wrapper for [`embedded_io::Read`] and [`embedded_io::Write`]

    #[cfg(test)]
    mod tests {
        use std::string::ToString;
        use std::{thread, vec::Vec};

        use crossbeam::channel::{Receiver, Sender, TryRecvError};

        use crate::builder::EditorBuilder;
        use crate::testlib::test_editor_with_case;
        use crate::testlib::{test_cases, MockTerminal};

        use super::*;

        struct MockSerial {
            rx: Receiver<u8>,
            buffer: Vec<u8>,
            tx: Sender<u8>,
        }

        impl MockSerial {
            fn new(tx: Sender<u8>, rx: Receiver<u8>) -> Self {
                Self {
                    rx,
                    buffer: Vec::new(),
                    tx,
                }
            }

            fn from_terminal(terminal: &mut MockTerminal) -> Self {
                let (tx, rx) = terminal.take_io();
                Self::new(tx.unwrap(), rx)
            }
        }

        impl serial::Read<u8> for MockSerial {
            type Error = ();

            fn read(&mut self) -> nb::Result<u8, Self::Error> {
                match self.rx.try_recv() {
                    Ok(byte) => Ok(byte),
                    Err(err) => match err {
                        TryRecvError::Empty => Err(nb::Error::WouldBlock),
                        TryRecvError::Disconnected => Err(nb::Error::Other(())),
                    },
                }
            }
        }

        impl serial::Write<u8> for MockSerial {
            type Error = ();

            fn write(&mut self, word: u8) -> nb::Result<(), Self::Error> {
                self.buffer.push(word);
                Ok(())
            }

            fn flush(&mut self) -> nb::Result<(), Self::Error> {
                for byte in self.buffer.drain(0..) {
                    self.tx.send(byte).unwrap();
                }

                Ok(())
            }
        }

        #[test]
        fn test_editor() {
            let prompt = "> ";

            for case in test_cases() {
                test_editor_with_case(
                    case,
                    prompt,
                    |term| MockSerial::from_terminal(term),
                    |serial, string_tx| {
                        thread::spawn(move || {
                            let mut io = IO::new(serial);
                            let mut editor = EditorBuilder::new_static::<100>()
                                .with_static_history::<128>()
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
}
