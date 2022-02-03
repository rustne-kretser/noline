use std::sync::Arc;

use noline::{line_buffer::AllocLineBuffer, no_sync::with_tokio::readline};
use termion::raw::IntoRawMode;
use tokio::{
    io::{self, AsyncWriteExt},
    sync::Mutex,
};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let _raw_term = std::io::stdout().into_raw_mode().unwrap();
    let stdin = Arc::new(Mutex::new(io::stdin()));
    let stdout = Arc::new(Mutex::new(io::stdout()));

    let prompt = "> ";

    loop {
        let mut buffer = AllocLineBuffer::new();

        if let Ok(line) = readline(&mut buffer, prompt, stdin.clone(), stdout.clone()).await {
            let s = format!("Read: '{}'\n\r", line);
            stdout.lock().await.write_all(s.as_bytes()).await.unwrap();
        } else {
            break;
        }
    }
}
