use core::future::Future;

use crate::common::{self, NolineInitializerState};
use crate::line_buffer::Buffer;
use crate::marker::Async;
use crate::output::OutputItem;

impl<'a, B: Buffer> common::NolineInitializer<'a, B, Async> {
    pub async fn initialize<IF, OF>(
        mut self,
        mut input: impl FnMut() -> IF,
        mut output: impl FnMut(&'a [u8]) -> OF,
    ) -> Result<Noline<'a, B>, ()>
    where
        IF: Future<Output = Result<u8, ()>>,
        OF: Future<Output = Result<(), ()>>,
    {
        output(self.prompt.as_bytes()).await?;
        output(self.init_bytes()).await?;

        let terminal = loop {
            if let NolineInitializerState::Done(terminal) = self.state {
                break terminal;
            }

            let byte = input().await?;
            self.advance(byte)?;
        };

        Ok(Noline::new(self.buffer, self.prompt, terminal))
    }
}

pub type NolineInitializer<'a, B> = common::NolineInitializer<'a, B, Async>;

impl<'a, B: Buffer> common::Noline<'a, B, Async> {
    pub async fn advance<F: Future<Output = Result<(), ()>>>(
        &mut self,
        input: u8,
        f: impl Fn(&[u8]) -> F,
    ) -> Option<Result<(), ()>> {
        for item in self.input_byte(input) {
            if let Some(bytes) = item.get_bytes() {
                if f(bytes).await.is_err() {
                    return Some(Err(()));
                }
            }

            match item {
                OutputItem::EndOfString => return Some(Ok(())),
                OutputItem::Abort => return Some(Err(())),
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
    ) -> Result<&'a str, ()> {
        let mut noline = NolineInitializer::new(buffer, prompt)
            .initialize(
                || async {
                    if let Ok(b) = stdin.lock().await.read_u8().await {
                        Ok(b)
                    } else {
                        Err(())
                    }
                },
                |bytes| async {
                    if stdout.lock().await.write_all(bytes).await.is_ok() {
                        stdout.lock().await.flush().await.or(Err(()))?;
                        Ok(())
                    } else {
                        Err(())
                    }
                },
            )
            .await?;

        stdout.lock().await.flush().await.or(Err(()))?;

        while let Ok(b) = stdin.lock().await.read_u8().await {
            match noline
                .advance(b, |output| {
                    // I know copying bytes to a vec isn't is bad, but
                    // after fighting lifetime issues with async
                    // closures just for the better part of an
                    // afternoon I just don't care anymore.
                    let output: Vec<u8> = output.iter().map(|&b| b).collect();

                    let stdout = stdout.clone();
                    async move {
                        if stdout
                            .lock()
                            .await
                            .write_all(output.as_slice())
                            .await
                            .is_ok()
                        {
                            Ok(())
                        } else {
                            Err(())
                        }
                    }
                })
                .await
            {
                Some(rc) => {
                    if rc.is_ok() {
                        return Ok(noline.buffer.as_str());
                    } else {
                        return Err(());
                    }
                }
                None => (),
            }

            stdout.lock().await.flush().await.or(Err(()))?;
        }

        Err(())
    }
}
