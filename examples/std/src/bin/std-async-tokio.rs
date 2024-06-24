use embedded_io_async::Write;
use noline::async_io::{
    async_std::{StdinWrapper, StdoutWrapper},
    IO,
};
use noline::builder::EditorBuilder;
use termion::raw::IntoRawMode;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let term_task = tokio::spawn(async {
        let _raw_term = std::io::stdout().into_raw_mode().unwrap();
        let mut stdin = StdinWrapper::default();
        let mut stdout = StdoutWrapper::default();
        let mut io = IO::new(&mut stdin, &mut stdout);

        let prompt = "> ";

        let mut editor = EditorBuilder::new_unbounded()
            .with_unbounded_history()
            .build_async(&mut io)
            .await
            .unwrap();

        while let Ok(line) = editor.readline(prompt, &mut io).await {
            let s = format!("Read: '{}'\n\r", line);
            io.output.write_all(s.as_bytes()).await.unwrap();
        }
    });

    match term_task.await {
        Ok(_) => (),
        Err(e) => eprintln!("Error: {:?}", e),
    }
}
