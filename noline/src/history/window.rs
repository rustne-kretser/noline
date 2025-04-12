use super::circ_index::CircularIndex;

pub(super) struct Window {
    size: usize,
    start: CircularIndex,
    end: CircularIndex,
}

impl Window {
    pub(super) fn new(size: usize) -> Self {
        let start = CircularIndex::new(0, size);
        let end = CircularIndex::new(0, size);
        Self { size, start, end }
    }

    pub(super) fn len(&self) -> usize {
        self.end.diff(self.start) as usize
    }

    pub(super) fn widen(&mut self) {
        self.end.increment();

        if self.end.diff(self.start) as usize > self.size {
            self.start.increment();
        }
    }

    pub(super) fn narrow(&mut self) {
        if self.end.diff(self.start) > 0 {
            self.start.increment();
        }
    }

    pub(super) fn start(&self) -> usize {
        self.start.index()
    }

    pub(super) fn end(&self) -> usize {
        self.end.index()
    }
}
