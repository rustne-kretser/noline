use embedded_io_async::Write;
use noline::builder::EditorBuilder;
use termion::raw::IntoRawMode;

use tokio::io;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub struct IOWrapper {
    stdin: io::Stdin,
    stdout: io::Stdout,
}
impl IOWrapper {
    pub fn new() -> Self {
        Self {
            stdin: io::stdin(),
            stdout: io::stdout(),
        }
    }
}

impl embedded_io_async::ErrorType for IOWrapper {
    type Error = embedded_io_async::ErrorKind;
}

impl embedded_io_async::Read for IOWrapper {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        self.stdin
            .read(buf)
            .await
            .map_err(|e| Self::Error::from(e.kind()))
    }
}

impl embedded_io_async::Write for IOWrapper {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.stdout.write(buf).await.map_err(|e| e.kind().into())
    }
    async fn flush(&mut self) -> Result<(), Self::Error> {
        self.stdout.flush().await.map_err(|e| e.kind().into())
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let term_task = tokio::spawn(async {
        let _raw_term = std::io::stdout().into_raw_mode().unwrap();
        let mut io = IOWrapper::new();

        let prompt = "> ";

        let mut editor = EditorBuilder::new_unbounded()
            .with_unbounded_history()
            .build_async(&mut io)
            .await
            .unwrap();

        while let Ok(line) = editor.readline(prompt, &mut io).await {
            let s = format!("Read: '{}'\n\r", line);
            io.stdout.write_all(s.as_bytes()).await.unwrap();
        }
    });

    match term_task.await {
        Ok(_) => (),
        Err(e) => eprintln!("Error: {:?}", e),
    }
}
