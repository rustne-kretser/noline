use core::fmt::Write as FmtWrite;
use defmt::info;
use embassy_futures::block_on;
use embassy_rp::usb::{Driver, Instance};
use embassy_usb::{
    class::cdc_acm::{ControlChanged, Receiver, Sender},
    driver::EndpointError,
};
use embedded_io_async::Write;
use fixed_queue::VecDeque;
use noline::{async_io::IO, builder::EditorBuilder};

fn map_error(_e: EndpointError) -> embedded_io_async::ErrorKind {
    embedded_io_async::ErrorKind::Other
}

// Implement the reader struct
struct Reader<'d, I: Instance> {
    stdin: &'d mut Receiver<'d, Driver<'d, I>>,
    queue: VecDeque<u8, 64>,
}

// Exposte the wait_connection function from the borrowd stdin
impl<'d, R: Instance> Writer<'d, R> {
    fn ready(&self) -> bool {
        self.stdout.rts() && self.stdout.dtr()
    }
    async fn wait_connection(&mut self) {
        self.stdout.wait_connection().await
    }
}

impl<'d, R: Instance> embedded_io_async::ErrorType for Reader<'d, R> {
    type Error = embedded_io_async::ErrorKind;
}

// Read data from the input and make it available asynchronously
impl<'d, R: Instance> embedded_io_async::Read for Reader<'d, R> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        // If the queue is empty
        while self.queue.is_empty() {
            let mut buf: [u8; 64] = [0; 64];
            // Read a maximum of 64 bytes from the ouput
            let len = self
                .stdin
                .read_packet(&mut buf)
                .await
                .map_err(|e| map_error(e))?;
            // This is safe because we only ever pull data when empty
            // And the queue has the same capacity as the input buffer
            for i in buf.iter().take(len) {
                let _ = self.queue.push_back(*i);
            }
        }

        if let Some(v) = self.queue.pop_front() {
            buf[0] = v;
            Ok(1)
        } else {
            Err(embedded_io_async::ErrorKind::Other)
        }
    }
}

// Simple writer structure
pub struct Writer<'d, I: Instance> {
    stdout: &'d mut Sender<'d, Driver<'d, I>>,
}
impl<'d, R: Instance> embedded_io_async::ErrorType for Writer<'d, R> {
    type Error = embedded_io_async::ErrorKind;
}

// Implement the noline writer trait to enable us to write to the USB output
impl<'d, R: Instance> embedded_io_async::Write for Writer<'d, R> {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        match self.stdout.write_packet(buf).await {
            Ok(()) => Ok(buf.len()),
            Err(_e) => Err(embedded_io_async::ErrorKind::Other),
        }
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        // TODO: Implement me
        Ok(())
    }
}

// We need to use the New Type idiom to enable us to implement FmtWrite for IO
struct MyIO<'a, R: embedded_io_async::Read, W: embedded_io_async::Write>(IO<'a, R, W>);
impl<'a, R: embedded_io_async::Read, W: embedded_io_async::Write> MyIO<'a, R, W> {
    fn new(read: &'a mut R, write: &'a mut W) -> Self {
        Self(IO::new(read, write))
    }
}

// Formatted output for the Writer which allows us to use the writeln! macro directly
impl<'a, R, W> FmtWrite for MyIO<'a, R, W>
where
    R: embedded_io_async::Read,
    W: embedded_io_async::Write,
{
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let data = s.as_bytes();

        block_on(self.0.write(data)).map_err(|_| core::fmt::Error {})?;

        Ok(())
    }
}

pub async fn cli<'d, T: Instance + 'd>(
    send: &'d mut Sender<'d, Driver<'d, T>>,
    recv: &'d mut Receiver<'d, Driver<'d, T>>,
    control: &'d mut ControlChanged<'d>,
) {
    let prompt = "> ";

    let mut stdin: Reader<'d, T> = Reader {
        stdin: recv,
        queue: VecDeque::new(),
    };
    let mut stdout: Writer<'d, T> = Writer { stdout: send };

    loop {
        stdout.wait_connection().await;

        while !stdout.ready() {
            control.control_changed().await;
        }

        let mut io = MyIO::new(&mut stdin, &mut stdout);

        let mut editor = EditorBuilder::new_static::<64>()
            .with_static_history::<8>()
            .build_async(&mut io.0)
            .await
            .unwrap();

        while let Ok(line) = editor.readline(prompt, &mut io.0).await {
            let _ = writeln!(io, "READ: {}", line);
        }
    }
}
