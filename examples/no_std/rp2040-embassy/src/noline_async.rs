use embassy_rp::usb::{Driver, Instance};
use embassy_usb::{
    class::cdc_acm::{ControlChanged, Receiver, Sender},
    driver::EndpointError,
};
use embedded_io_async::{ErrorKind, Write};
use fixed_queue::VecDeque;
use noline::builder::EditorBuilder;

struct IO<'a, T>
where
    T: Instance,
{
    pub stdin: &'a mut Receiver<'a, Driver<'a, T>>,
    queue: VecDeque<u8, 64>,
    pub stdout: &'a mut Sender<'a, Driver<'a, T>>,
}

impl<'a, T> IO<'a, T>
where
    T: Instance,
{
    fn new(
        stdin: &'a mut Receiver<'a, Driver<'a, T>>,
        stdout: &'a mut Sender<'a, Driver<'a, T>>,
    ) -> Self {
        Self {
            stdin,
            queue: VecDeque::new(),
            stdout,
        }
    }
}

#[derive(Debug)]
struct Error(());

impl From<EndpointError> for Error {
    fn from(_value: EndpointError) -> Self {
        Self(())
    }
}

impl embedded_io_async::Error for Error {
    fn kind(&self) -> ErrorKind {
        ErrorKind::Other
    }
}

impl<'a, T> embedded_io_async::ErrorType for IO<'a, T>
where
    T: Instance,
{
    type Error = Error;
}

// Read data from the input and make it available asynchronously
impl<'a, T> embedded_io_async::Read for IO<'a, T>
where
    T: Instance,
{
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        // If the queue is empty
        while self.queue.is_empty() {
            let mut buf: [u8; 64] = [0; 64];
            // Read a maximum of 64 bytes from the ouput
            let len = self.stdin.read_packet(&mut buf).await?;
            // This is safe because we only ever pull data when empty
            // And the queue has the same capacity as the input buffer
            for i in buf.iter().take(len) {
                self.queue.push_back(*i).expect("Buffer overflow");
            }
        }

        if let Some(v) = self.queue.pop_front() {
            buf[0] = v;
            Ok(1)
        } else {
            Err(Error(()))
        }
    }
}

// Implement the noline writer trait to enable us to write to the USB output
impl<'a, T> embedded_io_async::Write for IO<'a, T>
where
    T: Instance,
{
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.stdout.write_packet(buf).await?;

        Ok(buf.len())
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        // TODO: Implement me
        Ok(())
    }
}

const MAX_LINE_SIZE: usize = 64;

pub async fn cli<'d, T: Instance + 'd>(
    send: &'d mut Sender<'d, Driver<'d, T>>,
    recv: &'d mut Receiver<'d, Driver<'d, T>>,
    control: &'d mut ControlChanged<'d>,
) {
    let prompt = "> ";

    let mut io = IO::new(recv, send);
    let mut buffer = [0; MAX_LINE_SIZE];
    let mut history = [0; MAX_LINE_SIZE];

    loop {
        io.stdout.wait_connection().await;

        while !(io.stdout.rts() && io.stdout.dtr()) {
            control.control_changed().await;
        }

        let mut editor = EditorBuilder::from_slice(&mut buffer)
            .with_slice_history(&mut history)
            .build_async(&mut io)
            .await
            .unwrap();

        while let Ok(line) = editor.readline(prompt, &mut io).await {
            // Create a buffer that can take the MAX_LINE_SIZE along with the 'Read: ''\r/n' text
            let mut buf = [0u8; MAX_LINE_SIZE + 12];
            let s = format_no_std::show(&mut buf, format_args!("Read: '{}'\r\n", line))
                .expect("Format error");

            // split s into slices of MAX_LINE_SIZE bytes as the USB output buffer has a
            // maximum size that we will overflow if we try and write more than this at one time
            for chunk in s.as_bytes().chunks(MAX_LINE_SIZE) {
                io.write(chunk).await.expect("Write error");
            }
        }
    }
}
