#[derive(Debug)]
pub enum Error<RE, WE> {
    ParserError,
    Aborted,
    ReadError(RE),
    WriteError(WE),
}

impl<RE, WE> Error<RE, WE> {
    pub fn read_error<T>(err: RE) -> Result<T, Self> {
        Err(Self::ReadError(err))
    }

    pub fn write_error<T>(err: WE) -> Result<T, Self> {
        Err(Self::WriteError(err))
    }
}
