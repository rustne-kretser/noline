use noline::builder::EditorBuilder;
use termion::raw::IntoRawMode;
use noline::async_io::{IO, async_std::{StdinWrapper, StdoutWrapper}};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let _raw_term = std::io::stdout().into_raw_mode().unwrap();
    let mut io = IO::new(StdinWrapper::default(), StdoutWrapper::default());

    let prompt = "> ";

    let mut editor = EditorBuilder::new_unbounded()
        .with_unbounded_history()
        .build_async(&mut io)
        .await
        .unwrap();

    while let Ok(line) = editor.readline(prompt, &mut io).await {
        let s = format!("Read: '{}'\n\r", line);
        io.write(s.as_bytes()).await.unwrap();
    } 
}
