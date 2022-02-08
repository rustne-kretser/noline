use core::future::Future;

use crate::common;
use crate::error::Error;
use crate::line_buffer::Buffer;
use crate::marker::Async;
use crate::output::OutputItem;

impl<'a, B: Buffer> common::NolineInitializer<'a, B, Async> {
    pub async fn initialize<IF, OF, E>(
        mut self,
        mut input: impl FnMut() -> IF,
        mut output: impl FnMut(&'a [u8]) -> OF,
    ) -> Result<Noline<'a, B>, Error<E>>
    where
        IF: Future<Output = Result<u8, E>>,
        OF: Future<Output = Result<(), E>>,
    {
        output(self.clear_line()).await?;
        output(self.prompt.as_bytes()).await?;
        output(self.probe_size()).await?;

        let terminal = loop {
            let byte = input().await?;

            match self.advance(byte) {
                common::InitializerResult::Continue => (),
                common::InitializerResult::Item(terminal) => break terminal,
                common::InitializerResult::InvalidInput => return Err(Error::ParserError),
            }
        };

        Ok(Noline::new(self.buffer, self.prompt, terminal))
    }
}

pub type NolineInitializer<'a, B> = common::NolineInitializer<'a, B, Async>;

impl<'a, B: Buffer> common::Noline<'a, B, Async> {
    pub async fn advance<F, E>(
        &mut self,
        input: u8,
        f: impl Fn(&[u8]) -> F,
    ) -> Option<Result<(), Error<E>>>
    where
        F: Future<Output = Result<(), Error<E>>>,
    {
        for item in self.input_byte(input) {
            if let Some(bytes) = item.get_bytes() {
                if let Err(err) = f(bytes).await {
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
}

type Noline<'a, B> = common::Noline<'a, B, Async>;

#[cfg(feature = "tokio")]
pub mod with_tokio {
    use crate::line_buffer::LineBuffer;

    use super::*;

    use std::sync::Arc;
    use std::vec::Vec;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        sync::Mutex,
    };

    // I thought I had a pretty good handle on lifetimes, but async
    // lifetimes are something else. The Arc-Mutexes aren't my first
    // choice, but they make the borrow checker happy.
    pub async fn readline<'a, B: Buffer, W: AsyncWriteExt + Unpin, R: AsyncReadExt + Unpin>(
        buffer: &'a mut LineBuffer<B>,
        prompt: &'a str,
        stdin: Arc<Mutex<R>>,
        stdout: Arc<Mutex<W>>,
    ) -> Result<&'a str, Error<std::io::Error>> {
        let mut noline = NolineInitializer::new(buffer, prompt)
            .initialize(
                || async {
                    let b = stdin.lock().await.read_u8().await?;

                    Ok(b)
                },
                |bytes| async {
                    stdout.lock().await.write_all(bytes).await?;
                    stdout.lock().await.flush().await?;
                    Ok(())
                },
            )
            .await?;

        stdout.lock().await.flush().await?;

        loop {
            let b = stdin.lock().await.read_u8().await?;

            match noline
                .advance(b, |output| {
                    // I know copying bytes to a vec isn't is bad, but
                    // after fighting lifetime issues with async
                    // closures just for the better part of an
                    // afternoon I just don't care anymore.
                    let output: Vec<u8> = output.iter().map(|&b| b).collect();

                    let stdout = stdout.clone();
                    async move {
                        stdout.lock().await.write_all(output.as_slice()).await?;
                        Ok(())
                    }
                })
                .await
            {
                Some(rc) => {
                    rc?;

                    break Ok(noline.buffer.as_str());
                }
                None => (),
            }

            stdout.lock().await.flush().await?;
        }
    }
}
