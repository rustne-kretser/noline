//! Line editor for synchronous IO.
//!
//! The editor takes a struct implementing the [`Read`] and [`Write`]
//! traits. There are ready made implementations in [`std::IO`] and [`embedded::IO`].

use ::core::marker::PhantomData;

use crate::error::Error;
use crate::history::History;
use crate::line_buffer::{Buffer, LineBuffer};

use crate::core::{Initializer, InitializerResult, Line};
use crate::output::{Output, OutputItem};
use crate::terminal::Terminal;

/// Trait for reading bytes from input
pub trait Read {
    type Error;

    // Read single byte from input
    fn read(&mut self) -> Result<u8, Self::Error>;
}

/// Trait for writing bytes to output
pub trait Write {
    type Error;

    /// Write byte slice to output
    fn write(&mut self, buf: &[u8]) -> Result<(), Self::Error>;

    // Flush output
    fn flush(&mut self) -> Result<(), Self::Error>;
}

/// Line editor for synchronous IO
pub struct Editor<B: Buffer, H: History, IO: Read + Write> {
    buffer: LineBuffer<B>,
    terminal: Terminal,
    history: H,
    _marker: PhantomData<IO>,
}

impl<B, H, IO, RE, WE> Editor<B, H, IO>
where
    B: Buffer,
    H: History,
    IO: Read<Error = RE> + Write<Error = WE>,
{
    /// Create and initialize line editor
    pub fn new(io: &mut IO) -> Result<Self, Error<RE, WE>> {
        let mut initializer = Initializer::new();

        io.write(Initializer::init())
            .or_else(|err| Error::write_error(err))?;
        io.flush().or_else(|err| Error::write_error(err))?;

        let terminal = loop {
            let byte = io.read().or_else(|err| Error::read_error(err))?;

            match initializer.advance(byte) {
                InitializerResult::Continue => (),
                InitializerResult::Item(terminal) => break terminal,
                InitializerResult::InvalidInput => return Err(Error::ParserError),
            }
        };

        Ok(Self {
            buffer: LineBuffer::new(),
            terminal,
            history: H::default(),
            _marker: PhantomData,
        })
    }

    fn handle_output<'b>(output: Output<'b, B>, io: &mut IO) -> Result<Option<()>, Error<RE, WE>> {
        for item in output {
            if let Some(bytes) = item.get_bytes() {
                io.write(bytes).or_else(|err| Error::write_error(err))?;
            }

            io.flush().or_else(|err| Error::write_error(err))?;

            match item {
                OutputItem::EndOfString => return Ok(Some(())),
                OutputItem::Abort => return Err(Error::Aborted),
                _ => (),
            }
        }

        Ok(None)
    }

    /// Read line from `stdin`
    pub fn readline<'b>(
        &'b mut self,
        prompt: &'b str,
        io: &mut IO,
    ) -> Result<&'b str, Error<RE, WE>> {
        let mut line = Line::new(
            prompt,
            &mut self.buffer,
            &mut self.terminal,
            &mut self.history,
        );
        Self::handle_output(line.reset(), io)?;

        loop {
            let byte = io.read().or_else(|err| Error::read_error(err))?;
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
    pub fn get_history<'a>(&'a self) -> impl Iterator<Item = CircularSlice<'a>> {
        get_history_entries(&self.history)
    }
}

#[cfg(test)]
mod tests {
    use crate::history::NoHistory;

    use super::*;
    use ::std::string::ToString;
    use ::std::{thread, vec::Vec};
    use crossbeam::channel::{unbounded, Receiver, Sender};

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
            let mut editor: Editor<Vec<u8>, NoHistory, _> = Editor::new(&mut io).unwrap();

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

    use super::*;

    use ::std::io;
    use core::fmt;

    /// IO wrapper for stdin and stdout

    pub struct IO<R, W>
    where
        R: io::Read,
        W: io::Write,
    {
        input: R,
        output: W,
    }

    impl<R, W> IO<R, W>
    where
        R: io::Read,
        W: io::Write,
    {
        /// Create IO wrapper from input and output
        pub fn new(input: R, output: W) -> Self {
            Self { input, output }
        }

        /// Consume wrapper and return input and output as tuple
        pub fn take(self) -> (R, W) {
            (self.input, self.output)
        }
    }

    impl<R, W> Read for IO<R, W>
    where
        R: io::Read,
        W: io::Write,
    {
        type Error = std::io::Error;

        fn read(&mut self) -> Result<u8, Self::Error> {
            let mut buf = [0];
            self.input.read_exact(&mut buf)?;

            Ok(buf[0])
        }
    }

    impl<R, W> Write for IO<R, W>
    where
        R: io::Read,
        W: io::Write,
    {
        type Error = std::io::Error;

        fn write(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
            self.output.write_all(buf)
        }

        fn flush(&mut self) -> Result<(), Self::Error> {
            self.output.flush()
        }
    }

    impl<R, W> fmt::Write for IO<R, W>
    where
        R: io::Read,
        W: io::Write,
    {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            self.write(s.as_bytes()).or(Err(fmt::Error))
        }
    }

    #[cfg(test)]
    mod tests {
        use ::std::string::ToString;
        use ::std::{thread, vec::Vec};

        use crossbeam::channel::{unbounded, Receiver, Sender};

        use crate::history::NoHistory;
        use crate::sync::Editor;
        use crate::testlib::{test_cases, test_editor_with_case, MockTerminal};
        use ::std::io::Read as IoRead;

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
                            let mut editor = Editor::<Vec<u8>, NoHistory, _>::new(&mut io).unwrap();

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
    //! Implementation for embedded systems. Requires feature `embedded`.

    use core::{
        fmt,
        ops::{Deref, DerefMut},
    };

    use embedded_hal::serial;
    use nb::block;

    use super::*;

    pub struct IO<RW>
    where
        RW: serial::Read<u8> + serial::Write<u8>,
    {
        rw: RW,
    }

    impl<RW> IO<RW>
    where
        RW: serial::Read<u8> + serial::Write<u8>,
    {
        pub fn new(rw: RW) -> Self {
            Self { rw }
        }

        pub fn take(self) -> RW {
            self.rw
        }
    }

    impl<RW> Read for IO<RW>
    where
        RW: serial::Read<u8> + serial::Write<u8>,
    {
        type Error = <RW as serial::Read<u8>>::Error;

        fn read(&mut self) -> Result<u8, Self::Error> {
            block!(self.rw.read())
        }
    }

    impl<RW> Write for IO<RW>
    where
        RW: serial::Read<u8> + serial::Write<u8>,
    {
        type Error = <RW as serial::Write<u8>>::Error;

        fn write(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
            for &b in buf {
                block!(self.rw.write(b))?;
            }

            Ok(())
        }

        fn flush(&mut self) -> Result<(), Self::Error> {
            block!(self.rw.flush())
        }
    }

    impl<RW> fmt::Write for IO<RW>
    where
        RW: serial::Read<u8> + serial::Write<u8>,
    {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            self.write(s.as_bytes()).or(Err(fmt::Error))
        }
    }

    impl<RW> Deref for IO<RW>
    where
        RW: serial::Read<u8> + serial::Write<u8>,
    {
        type Target = RW;

        fn deref(&self) -> &Self::Target {
            &self.rw
        }
    }

    impl<RW> DerefMut for IO<RW>
    where
        RW: serial::Read<u8> + serial::Write<u8>,
    {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.rw
        }
    }

    #[cfg(test)]
    mod tests {
        use ::std::string::ToString;
        use ::std::{thread, vec::Vec};

        use crossbeam::channel::{Receiver, Sender, TryRecvError};

        use crate::history::NoHistory;
        use crate::line_buffer::StaticBuffer;
        use crate::sync::Editor;
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
                            let mut editor =
                                Editor::<StaticBuffer<100>, NoHistory, _>::new(&mut io).unwrap();

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
