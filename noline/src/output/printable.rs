pub(super) enum PrintableItem<'a> {
    Str(&'a str),
    Newline,
}

pub(super) struct Printable<'a, I> {
    s: &'a str,
    newline: bool,
    iter: Option<I>,
}

impl<'a, 'item, I> Printable<'a, I>
where
    I: Iterator<Item = &'item str>,
    'item: 'a,
{
    pub(super) fn from_str(s: &'a str) -> Self {
        Self {
            s,
            newline: false,
            iter: None,
        }
    }

    pub(super) fn from_iter(iter: I) -> Self {
        Self {
            s: "",
            newline: false,
            iter: Some(iter),
        }
    }

    pub(super) fn next_item(&mut self, max_chars: usize) -> Option<PrintableItem<'a>> {
        if self.newline {
            self.newline = false;
            Some(PrintableItem::Newline)
        } else {
            let s = if self.s.is_empty() {
                if let Some(iter) = &mut self.iter {
                    iter.next()?
                } else {
                    return None;
                }
            } else {
                self.s
            };

            let split_at_char = max_chars.min(s.chars().count());
            let split_at_byte = s
                .char_indices()
                .nth(split_at_char)
                .map(|(index, _)| index)
                .unwrap_or(s.len());

            let (s, rest) = s.split_at(split_at_byte);

            if split_at_char == max_chars {
                self.newline = true
            }

            self.s = rest;
            Some(PrintableItem::Str(s))
        }
    }
}
