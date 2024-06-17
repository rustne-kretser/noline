use noline::async_io::{
    async_std::{StdinWrapper, StdoutWrapper},
    IO,
};
use noline::builder::EditorBuilder;
use std::fmt::Write as FmtWrite;
use termion::raw::IntoRawMode;
use tokio::{runtime::Handle, task};

struct MyIO<'a, R: embedded_io_async::Read, W: embedded_io_async::Write>(IO<'a, R, W>);
impl<'a, R: embedded_io_async::Read, W: embedded_io_async::Write> MyIO<'a, R, W> {
    fn new(stdin: &'a mut R, stdout: &'a mut W) -> Self {
        Self(IO::new(stdin, stdout))
    }
}

impl<'a, R: embedded_io_async::Read, W: embedded_io_async::Write> FmtWrite for MyIO<'a, R, W> {
    fn write_str(&mut self, s: &str) -> Result<(), std::fmt::Error> {
        let data = s.as_bytes();
        let ret = task::block_in_place(move || {
            Handle::current()
                .block_on(async { self.0.write(data).await.map_err(|_| core::fmt::Error {}) })
        });

        ret.map_err(|_| core::fmt::Error {})
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let term_task = tokio::spawn(async {
        let _raw_term = std::io::stdout().into_raw_mode().unwrap();
        let mut stdin = StdinWrapper::default();
        let mut stdout = StdoutWrapper::default();
        let mut io = MyIO::new(&mut stdin, &mut stdout);

        let prompt = "> ";

        let mut editor = EditorBuilder::new_unbounded()
            .with_unbounded_history()
            .build_async(&mut io.0)
            .await
            .unwrap();

        while let Ok(line) = editor.readline(prompt, &mut io.0).await {
            writeln!(io, "Read: '{}'", line).unwrap();
        }
    });

    let _ = term_task.await;
}
