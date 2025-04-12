use super::{circ_range::CircularRange, window::Window, CircularSlice, History};

/// Static history backed by array
pub struct SliceHistory<'a> {
    buffer: &'a mut [u8],
    pub(super) window: Window,
}

impl<'a> SliceHistory<'a> {
    /// Create new static history
    pub fn new(buffer: &'a mut [u8]) -> Self {
        Self {
            window: Window::new(buffer.len()),
            buffer,
        }
    }

    pub(super) fn get_available_range(&self) -> CircularRange {
        let len = self.buffer.len();
        CircularRange::new(self.window.end(), self.window.end(), len, len)
    }

    fn get_buffer(&self) -> CircularSlice<'_> {
        CircularSlice::new(
            self.buffer,
            self.window.start(),
            self.window.end(),
            self.window.len(),
        )
    }

    fn get_entry_ranges(&self) -> impl Iterator<Item = CircularRange> + '_ {
        let delimeters =
            self.get_buffer()
                .into_iter()
                .filter_map(|(index, b)| if *b == 0x0 { Some(index) } else { None });

        [self.window.start()]
            .into_iter()
            .chain(delimeters.clone().map(|i| i + 1))
            .zip(delimeters.chain([self.window.end()]))
            .filter_map(|(start, end)| {
                if start != end {
                    Some(CircularRange::new(
                        start,
                        end,
                        self.window.len(),
                        self.buffer.len(),
                    ))
                } else {
                    None
                }
            })
    }

    pub(super) fn get_entries(&self) -> impl Iterator<Item = CircularSlice<'_>> {
        self.get_entry_ranges()
            .map(|range| CircularSlice::from_range(self.buffer, range))
    }
}

impl<'a> History for SliceHistory<'a> {
    fn add_entry<'b>(&mut self, entry: &'b str) -> Result<(), &'b str> {
        if entry.len() + 1 > self.buffer.len() {
            return Err(entry);
        }

        for (_, b) in self
            .get_available_range()
            .into_iter()
            .zip(entry.as_bytes().iter())
        {
            self.buffer[self.window.end()] = *b;
            self.window.widen();
        }

        if self.buffer[self.window.end()] != 0x0 {
            self.buffer[self.window.end()] = 0x0;

            self.window.widen();

            while self.buffer[self.window.start()] != 0x0 {
                self.window.narrow();
            }
        } else {
            self.window.widen();
        }

        Ok(())
    }

    fn number_of_entries(&self) -> usize {
        self.get_entries().count()
    }

    fn get_entry(&self, index: usize) -> Option<CircularSlice<'_>> {
        self.get_entries().nth(index)
    }
}
