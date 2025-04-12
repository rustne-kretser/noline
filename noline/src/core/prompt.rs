use super::StrIter;

#[cfg_attr(test, derive(Debug))]
pub struct Prompt<I> {
    parts: I,
    len: usize,
}

impl<'a, I> Prompt<I>
where
    I: Iterator<Item = &'a str> + Clone,
{
    fn new(parts: I) -> Self {
        Self {
            len: parts.clone().map(|part| part.len()).sum(),
            parts,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }
}

impl<'a, I> Prompt<I>
where
    I: Iterator<Item = &'a str> + Clone,
{
    pub fn iter(&self) -> I {
        self.parts.clone()
    }
}

impl<'a> From<&'a str> for Prompt<StrIter<'a>> {
    fn from(value: &'a str) -> Self {
        Self::new(StrIter { s: Some(value) })
    }
}

impl<'a, I> From<I> for Prompt<I>
where
    I: Iterator<Item = &'a str> + Clone,
{
    fn from(value: I) -> Self {
        Self::new(value)
    }
}
