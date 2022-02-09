#[derive(Debug)]
pub enum Error<E> {
    ParserError,
    Aborted,
    IoError(E),
}

impl<E> From<E> for Error<E> {
    fn from(err: E) -> Self {
        Self::IoError(err)
    }
}
