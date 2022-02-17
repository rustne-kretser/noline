use crate::error::Error;
use crate::line_buffer::Buffer;
use crate::marker::Sync;

use crate::common;
use crate::output::{Output, OutputItem};

impl<'a, B: Buffer> common::NolineInitializer<'a, B, Sync> {
    pub fn initialize<IE, OE>(
        mut self,
        mut input: impl FnMut() -> Result<u8, Error<IE, OE>>,
        mut output: impl FnMut(&'a [u8]) -> Result<(), Error<IE, OE>>,
    ) -> Result<Noline<'a, B>, Error<IE, OE>> {
        output(self.init())?;

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
    pub fn handle_ouput<'b, F, IE, OE>(
        output: Output<'b, B>,
        mut f: F,
    ) -> Option<Result<(), Error<IE, OE>>>
    where
        F: FnMut(&[u8]) -> Result<(), Error<IE, OE>>,
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

    pub fn advance<'b, F, IE, OE>(
        &'b mut self,
        input: u8,
        f: F,
    ) -> Option<Result<(), Error<IE, OE>>>
    where
        F: FnMut(&[u8]) -> Result<(), Error<IE, OE>>,
    {
        Self::handle_ouput(self.input_byte(input), f)
    }

    pub fn reset<'b, F, IE, OE>(&'b mut self, f: F) -> Result<(), Error<IE, OE>>
    where
        F: FnMut(&[u8]) -> Result<(), Error<IE, OE>>,
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
    ) -> impl FnMut(&[u8]) -> Result<(), Error<std::io::Error, std::io::Error>> + 'b {
        |bytes| {
            stdout
                .write_all(bytes)
                .or_else(|err| Error::write_error(err))?;
            stdout.flush().or_else(|err| Error::write_error(err))?;
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
        ) -> Result<Self, Error<std::io::Error, std::io::Error>> {
            let noline = NolineInitializer::new(prompt).initialize(
                || {
                    let b = stdin
                        .bytes()
                        .next()
                        .unwrap_or_else(|| unreachable!())
                        .or_else(|err| Error::read_error(err))?;
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
        ) -> Result<&'b str, Error<std::io::Error, std::io::Error>> {
            let mut f = output_closure(stdout);
            self.noline.reset(&mut f)?;

            loop {
                let byte = stdin
                    .bytes()
                    .next()
                    .unwrap_or_else(|| unreachable!())
                    .or_else(|err| Error::read_error(err))?;
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

        impl Read for MockStdin {
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

        impl Write for MockStdout {
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
        fn test_editor() {
            let prompt = "> ";

            for case in test_cases() {
                test_editor_with_case(
                    case,
                    prompt,
                    |term| MockIO::from_terminal(term).get_pipes(),
                    |(mut stdin, mut stdout), string_tx| {
                        let mut editor =
                            Editor::<Vec<u8>>::new(prompt, &mut stdin, &mut stdout).unwrap();
                        thread::spawn(move || {
                            while let Ok(s) = editor.readline(&mut stdin, &mut stdout) {
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
    use core::cell::RefCell;

    use super::*;
    use embedded_hal::serial::{Read, Write};
    use nb::block;

    fn write_bytes<W, RE, WE>(bytes: &[u8], tx: &mut W) -> Result<(), Error<RE, WE>>
    where
        W: Write<u8, Error = WE>,
    {
        for b in bytes {
            block!(tx.write(*b)).or_else(|err| Error::write_error(err))?;
        }

        block!(tx.flush()).or_else(|err| Error::write_error(err))?;

        Ok(())
    }

    fn output_closure<'b, W, RE, WE>(
        tx: &'b mut W,
    ) -> impl FnMut(&[u8]) -> Result<(), Error<RE, WE>> + 'b
    where
        W: Write<u8, Error = WE>,
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
        pub fn new<S, RE, WE>(prompt: &'a str, serial: &mut S) -> Result<Self, Error<RE, WE>>
        where
            S: Write<u8, Error = WE> + Read<u8, Error = RE>,
        {
            let serial = RefCell::new(serial);

            let noline = NolineInitializer::new(prompt).initialize(
                || Ok(block!(serial.borrow_mut().read()).or_else(|err| Error::read_error(err))?),
                |bytes| {
                    let mut tx = serial.borrow_mut();

                    write_bytes(bytes, *tx)?;
                    Ok(())
                },
            )?;

            Ok(Self { noline })
        }

        pub fn readline<'b, S, RE, WE>(
            &'b mut self,
            serial: &mut S,
        ) -> Result<&'b str, Error<RE, WE>>
        where
            S: Write<u8, Error = WE> + Read<u8, Error = RE>,
        {
            self.noline.reset(output_closure(serial))?;

            loop {
                let byte = block!(serial.read()).or_else(|err| Error::read_error(err))?;

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

        use crossbeam::channel::{Receiver, Sender, TryRecvError};

        use crate::line_buffer::StaticBuffer;
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

        impl Read<u8> for MockSerial {
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

        impl Write<u8> for MockSerial {
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
                    |mut serial, string_tx| {
                        let mut editor =
                            Editor::<StaticBuffer<100>>::new(prompt, &mut serial).unwrap();
                        thread::spawn(move || {
                            while let Ok(s) = editor.readline(&mut serial) {
                                string_tx.send(s.to_string()).unwrap();
                            }
                        })
                    },
                )
            }
        }
    }
}
