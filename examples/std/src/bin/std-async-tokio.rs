use noline::builder::EditorBuilder;
use termion::raw::IntoRawMode;
use tokio::io::{self, AsyncWriteExt};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let _raw_term = std::io::stdout().into_raw_mode().unwrap();
    let mut stdin = io::stdin();
    let mut stdout = io::stdout();

    let prompt = "> ";

    let mut editor = EditorBuilder::new_unbounded()
        .with_unbounded_history()
        .build_async_tokio(&mut stdin, &mut stdout)
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
