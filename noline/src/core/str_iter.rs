#[derive(Clone)]
pub struct StrIter<'a> {
    pub(super) s: Option<&'a str>,
}

impl<'a> Iterator for StrIter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        self.s.take()
    }
}
