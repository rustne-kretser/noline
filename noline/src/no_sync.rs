use core::future::Future;

use crate::common::{self, NolineInitializerState};
use crate::line_buffer::Buffer;
use crate::marker::Async;

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
    pub async fn advance<'b, F: Future<Output = Result<(), ()>>>(
        &'b mut self,
        input: u8,
        mut f: impl FnMut(&[u8]) -> F,
    ) -> Option<Result<(), ()>> {
        let mut status = self.input_byte(input);

        if let Some(bytes) = status.iter_bytes() {
            for bytes in bytes {
                if f(bytes.as_bytes()).await.is_err() {
                    return Some(Err(()));
                }
            }
        }

        status.is_done()
    }
}

type Noline<'a, B> = common::Noline<'a, B, Async>;
