use crate::error::Error;
use crate::line_buffer::Buffer;
use crate::marker::Sync;

use crate::common;
use crate::output::{Output, OutputItem};

impl<'a, B: Buffer> common::NolineInitializer<'a, B, Sync> {
    pub fn initialize<E>(
        mut self,
        mut input: impl FnMut() -> Result<u8, Error<E>>,
        mut output: impl FnMut(&'a [u8]) -> Result<(), Error<E>>,
    ) -> Result<Noline<'a, B>, Error<E>> {
        output(self.clear_line())?;
        output(self.prompt.as_bytes())?;
        output(self.probe_size())?;

        let terminal = loop {
            let byte = input()?;

            match self.advance(byte) {
                common::InitializerResult::Continue => (),
                common::InitializerResult::Item(terminal) => break terminal,
                common::InitializerResult::InvalidInput => return Err(Error::ParserError),
            }
        };

        Ok(Noline::new(self.prompt, terminal))
    }
}

pub type NolineInitializer<'a, B> = common::NolineInitializer<'a, B, Sync>;

impl<'a, B: Buffer> common::Noline<'a, B, Sync> {
    pub fn handle_ouput<'b, F, E>(output: Output<'b, B>, mut f: F) -> Option<Result<(), Error<E>>>
    where
        F: FnMut(&[u8]) -> Result<(), Error<E>>,
    {
        for item in output {
            if let Some(bytes) = item.get_bytes() {
                if let Err(err) = f(bytes) {
                    return Some(Err(err));
                }
            }

            match item {
                OutputItem::EndOfString => return Some(Ok(())),
                OutputItem::Abort => return Some(Err(Error::Aborted)),
                _ => (),
            }
        }

        None
    }

    pub fn advance<'b, F, E>(&'b mut self, input: u8, f: F) -> Option<Result<(), Error<E>>>
    where
        F: FnMut(&[u8]) -> Result<(), Error<E>>,
    {
        Self::handle_ouput(self.input_byte(input), f)
    }

    pub fn reset<'b, F, E>(&'b mut self, f: F) -> Result<(), Error<E>>
    where
        F: FnMut(&[u8]) -> Result<(), Error<E>>,
    {
        if let Some(res) = Self::handle_ouput(self.reset_line(), f) {
            res?;
        }

        Ok(())
    }
}

pub type Noline<'a, B> = common::Noline<'a, B, Sync>;

#[cfg(any(test, feature = "std"))]
pub mod with_std {
    use super::*;
    use std::io::Read;
    use std::io::Write;

    pub struct Editor<'a, B>
    where
        B: Buffer,
    {
        noline: Noline<'a, B>,
    }

    fn output_closure<'b, W: Write>(
        stdout: &'b mut W,
    ) -> impl FnMut(&[u8]) -> Result<(), Error<std::io::Error>> + 'b {
        |bytes| {
            stdout.write_all(bytes)?;
            stdout.flush()?;
            Ok(())
        }
    }

    impl<'a, B> Editor<'a, B>
    where
        B: Buffer,
    {
        pub fn new<W: Write, R: Read>(
            prompt: &'a str,
            stdin: &mut R,
            stdout: &mut W,
        ) -> Result<Self, Error<std::io::Error>> {
            let noline = NolineInitializer::new(prompt).initialize(
                || {
                    let b = stdin.bytes().next().unwrap_or_else(|| unreachable!())?;
                    Ok(b)
                },
                output_closure(stdout),
            )?;

            Ok(Self { noline })
        }

        pub fn readline<'b, W: Write, R: Read>(
            &'b mut self,
            stdin: &mut R,
            stdout: &mut W,
        ) -> Result<&'b str, Error<std::io::Error>> {
            let mut f = output_closure(stdout);
            self.noline.reset(&mut f)?;

            loop {
                let byte = stdin.bytes().next().unwrap_or_else(|| unreachable!())?;
                match self.noline.advance(byte, &mut f) {
                    Some(rc) => {
                        rc?;

                        break Ok(self.noline.buffer.as_str());
                    }
                    None => (),
                }
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use std::string::ToString;
        use std::{thread, vec::Vec};

        use crossbeam::channel::{Receiver, Sender};

        use crate::testlib::{test_cases, test_editor_with_case, MockTerminal};

        use super::*;

        struct MockStdout {
            buffer: Vec<u8>,
            tx: Sender<Option<u8>>,
        }

        impl MockStdout {
            fn new(tx: Sender<Option<u8>>) -> Self {
                Self {
                    buffer: Vec::new(),
                    tx,
                }
            }

            fn send_eof(&self) {
                self.tx.send(None).unwrap();
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

            fn from_terminal(terminal: &MockTerminal) -> Self {
                Self::new(
                    MockStdin::new(terminal.keyboard_rx.clone()),
                    MockStdout::new(terminal.terminal_tx.clone()),
                )
            }

            fn get_pipes(self) -> (MockStdin, MockStdout) {
                (self.stdin, self.stdout)
            }
        }

        impl Read for MockStdin {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                for i in 0..(buf.len()) {
                    if let Ok(byte) = self.rx.recv() {
                        buf[i] = byte;
                    }
                }

                Ok(buf.len())
            }
        }

        impl Write for MockStdout {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.buffer.extend(buf);
                Ok(buf.len())
            }

            fn flush(&mut self) -> std::io::Result<()> {
                for byte in self.buffer.drain(0..) {
                    self.tx.send(Some(byte)).unwrap();
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
                    |term| MockIO::from_terminal(&term).get_pipes(),
                    |(mut stdin, mut stdout)| {
                        let mut editor =
                            Editor::<Vec<u8>>::new(prompt, &mut stdin, &mut stdout).unwrap();
                        thread::spawn(move || {
                            if let Ok(s) = editor.readline(&mut stdin, &mut stdout) {
                                stdout.send_eof();
                                Some(s.to_string())
                            } else {
                                None
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
    use core::cell::RefCell;

    use super::*;
    use embedded_hal::serial::{Read, Write};
    use nb::block;

    fn write_bytes<W, E>(bytes: &[u8], tx: &mut W) -> Result<(), Error<E>>
    where
        W: Write<u8, Error = E>,
    {
        for b in bytes {
            block!(tx.write(*b))?;
        }

        block!(tx.flush())?;

        Ok(())
    }

    fn output_closure<'b, W, E>(tx: &'b mut W) -> impl FnMut(&[u8]) -> Result<(), Error<E>> + 'b
    where
        W: Write<u8, Error = E>,
    {
        |bytes| write_bytes(bytes, tx)
    }

    pub struct Editor<'a, B>
    where
        B: Buffer,
    {
        noline: Noline<'a, B>,
    }

    impl<'a, B> Editor<'a, B>
    where
        B: Buffer,
    {
        pub fn new<S, E>(prompt: &'a str, serial: &mut S) -> Result<Self, Error<E>>
        where
            S: Write<u8, Error = E> + Read<u8, Error = E>,
        {
            let serial = RefCell::new(serial);

            let noline = NolineInitializer::new(prompt).initialize(
                || Ok(block!(serial.borrow_mut().read())?),
                |bytes| {
                    let mut tx = serial.borrow_mut();

                    write_bytes(bytes, *tx)
                },
            )?;

            Ok(Self { noline })
        }

        pub fn readline<'b, S, E>(&'b mut self, serial: &mut S) -> Result<&'b str, Error<E>>
        where
            S: Write<u8, Error = E> + Read<u8, Error = E>,
        {
            self.noline.reset(output_closure(serial))?;

            loop {
                let byte = block!(serial.read())?;

                match self.noline.advance(byte, output_closure(serial)) {
                    Some(rc) => {
                        rc?;

                        break Ok(self.noline.buffer.as_str());
                    }
                    None => (),
                }
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use std::string::ToString;
        use std::{thread, vec::Vec};

        use crossbeam::channel::{Receiver, Sender};

        use crate::line_buffer::StaticBuffer;
        use crate::testlib::test_editor_with_case;
        use crate::testlib::{test_cases, MockTerminal};

        use super::*;

        struct MockSerial {
            rx: Receiver<u8>,
            buffer: Vec<u8>,
            tx: Sender<Option<u8>>,
        }

        impl MockSerial {
            fn new(tx: Sender<Option<u8>>, rx: Receiver<u8>) -> Self {
                Self {
                    rx,
                    buffer: Vec::new(),
                    tx,
                }
            }

            fn from_terminal(terminal: &MockTerminal) -> Self {
                Self::new(terminal.terminal_tx.clone(), terminal.keyboard_rx.clone())
            }

            fn send_eof(&mut self) {
                self.tx.send(None).unwrap();
            }
        }

        impl Read<u8> for MockSerial {
            type Error = ();

            fn read(&mut self) -> nb::Result<u8, Self::Error> {
                if let Ok(byte) = self.rx.try_recv() {
                    Ok(byte)
                } else {
                    Err(nb::Error::WouldBlock)
                }
            }
        }

        impl Write<u8> for MockSerial {
            type Error = ();

            fn write(&mut self, word: u8) -> nb::Result<(), Self::Error> {
                self.buffer.push(word);
                Ok(())
            }

            fn flush(&mut self) -> nb::Result<(), Self::Error> {
                for byte in self.buffer.drain(0..) {
                    self.tx.send(Some(byte)).unwrap();
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
                    |mut serial| {
                        let mut editor =
                            Editor::<StaticBuffer<100>>::new(prompt, &mut serial).unwrap();
                        thread::spawn(move || {
                            if let Ok(s) = editor.readline(&mut serial) {
                                serial.send_eof();
                                Some(s.to_string())
                            } else {
                                None
                            }
                        })
                    },
                )
            }
        }
    }
}
