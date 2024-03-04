use noline::no_sync::async_trait::{AsyncEditor, NolineAsyncRead, NolineAsyncWrite};
use noline::error::Error;
use noline::history::StaticHistory;
use noline::line_buffer::StaticBuffer;

use termion::raw::IntoRawMode;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};


struct TokioReader<'a, R: AsyncReadExt + Unpin> {
    stdin: &'a mut R,
}

impl<R: AsyncReadExt + Unpin> NolineAsyncRead<std::io::Error, std::io::Error>
    for TokioReader<'_, R>
{
    async fn read(&mut self) -> Result<u8, Error<std::io::Error, std::io::Error>> {
        Ok(self
            .stdin
            .read_u8()
            .await
            .or_else(|err| Error::read_error(err))?)
    }
}

struct TokioWriter<W: AsyncWriteExt + Unpin> {
    stdout: W,
}

// impl<W: AsyncWriteExt + Unpin> TokioWriter<W> {
//     fn write_all(&mut self, buf: &[u8]) -> Result<()> {
//         self.stdout.write_all(buf)
//     }
// }

impl<W: AsyncWriteExt + Unpin> NolineAsyncWrite<std::io::Error, std::io::Error> for TokioWriter<W> {
    async fn write(&mut self, buf: &[u8]) -> Result<(), Error<std::io::Error, std::io::Error>> {
        self.stdout
            .write_all(buf)
            .await
            .or_else(|err| Error::write_error(err))?;
        Ok(())
    }
    async fn flush(&mut self) -> Result<(), Error<std::io::Error, std::io::Error>> {
        self.stdout
            .flush()
            .await
            .or_else(|err| Error::write_error(err))?;
        Ok(())
    }
}



#[tokio::main(flavor = "current_thread")]
async fn main() {
    let _raw_term = std::io::stdout().into_raw_mode().unwrap();
    let mut stdin = TokioReader { stdin: &mut io::stdin() };
    let mut stdout = TokioWriter { stdout: &mut io::stdout() };

    let prompt = "> ";

    let mut editor = AsyncEditor::<StaticBuffer<64>, StaticHistory<64>, std::io::Error, std::io::Error>::new(&mut stdin, &mut stdout).await.unwrap();


    loop {
        if let Ok(line) = editor.readline(prompt, &mut stdin, &mut stdout).await {
            let s = format!("Read: '{}'\n\r", line);
            stdout.write(s.as_bytes()).await.unwrap();
        } else {
            break;
        }
    }
}
