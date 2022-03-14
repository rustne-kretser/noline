use noline::no_sync::tokio::Editor;
use termion::raw::IntoRawMode;
use tokio::io::{self, AsyncWriteExt};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let _raw_term = std::io::stdout().into_raw_mode().unwrap();
    let mut stdin = io::stdin();
    let mut stdout = io::stdout();

    let prompt = "> ";

    let mut editor = Editor::<Vec<u8>>::new(&mut stdin, &mut stdout)
        .await
        .unwrap();

    loop {
        if let Ok(line) = editor.readline(prompt, &mut stdin, &mut stdout).await {
            let s = format!("Read: '{}'\n\r", line);
            stdout.write_all(s.as_bytes()).await.unwrap();
        } else {
            break;
        }
    }
}
