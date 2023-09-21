use core::fmt::Write as FmtWrite;

use defmt::info;
use embassy_futures::block_on;
use embassy_rp::usb::{Driver, Instance};
use embassy_usb::{
    class::cdc_acm::{Receiver, Sender},
    driver::EndpointError,
};
use fixed_queue::VecDeque;
use noline::{
    error::Error,
    no_sync::async_trait::{AsyncEditor, NolineAsyncRead, NolineAsyncWrite},
    history::StaticHistory,
    line_buffer::StaticBuffer,
};

// Implement the reader struct
struct Reader<'d, I: Instance> {
    stdin: &'d mut Receiver<'d, Driver<'d, I>>,
    queue: VecDeque<u8, 64>,
}

// Exposte the wait_connection function from the borrowd stdin
impl<'d, R: Instance> Reader<'d, R> {
    async fn wait_connection(&mut self) {
        self.stdin.wait_connection().await
    }
}

// Read data from the input and make it available asynchronously
impl<'d, R: Instance> NolineAsyncRead<EndpointError, EndpointError> for Reader<'d, R> {
    async fn read(&mut self) -> Result<u8, Error<EndpointError, EndpointError>> {
        // If the queue is empty
        while self.queue.is_empty() {
            let mut buf: [u8; 64] = [0; 64];
            // Read a maximum of 64 bytes from the ouput
            match self.stdin.read_packet(&mut buf).await {
                Ok(len) => {
                    // This is safe because we only ever pull data when empty
                    // And the queue has the same capacity as the input buffer
                    for i in buf.iter().take(len) {
                        let _ = self.queue.push_back(*i);
                    }
                }
                Err(e) => return Err(Error::ReadError(e)),
            }
        }

        if let Some(v) = self.queue.pop_front() {
            Ok(v)
        } else {
            Err(Error::Aborted)
        }
    }
}

// Simple writer structure
pub struct Writer<'d, I: Instance> {
    stdout: &'d mut Sender<'d, Driver<'d, I>>,
}

// Implement the noline writer trait to enable us to write to the USB output
impl<'d, R: Instance> NolineAsyncWrite<EndpointError, EndpointError> for Writer<'d, R> {
    async fn write(&mut self, buf: &[u8]) -> Result<(), Error<EndpointError, EndpointError>> {
        match self.stdout.write_packet(buf).await {
            Ok(()) => Ok(()),
            Err(e) => Err(Error::WriteError(e)),
        }
    }

    async fn flush(&mut self) -> Result<(), Error<EndpointError, EndpointError>> {
        // TODO: Implement me
        Ok(())
    }
}

// Formatted output for the Writer which allows us to use the writeln! macro directly
impl<'d, W: Instance> FmtWrite for Writer<'d, W> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let data = s.as_bytes();

        block_on(self.write(data)).map_err(|_| core::fmt::Error {})?;

        Ok(())
    }
}

pub async fn cli<'d, T: Instance + 'd>(
    send: &'d mut Sender<'d, Driver<'d, T>>,
    recv: &'d mut Receiver<'d, Driver<'d, T>>,
) {
    let prompt = "> ";

    let mut stdin: Reader<'d, T> = Reader {
        stdin: recv,
        queue: VecDeque::new(),
    };
    let mut stdout: Writer<'d, T> = Writer { stdout: send };

    loop {
        stdin.wait_connection().await;
        info!("Connected");

        let mut editor =
            AsyncEditor::<StaticBuffer<64>, StaticHistory<8>, EndpointError, EndpointError>::new(
                &mut stdin,
                &mut stdout,
            )
            .await
            .unwrap();

        if let Ok(line) = editor.readline(prompt, &mut stdin, &mut stdout).await {
            match writeln!(stdout, "READ: {}", line) {
                Ok(()) => {},
                Err(_e) => {},
            }
        }
    }
}
