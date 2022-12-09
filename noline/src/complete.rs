use crate::line_buffer::{Buffer, LineBuffer};

pub trait Completer {
    fn complete(&self, line: &str, idx: usize) -> Option<&str>;
}

impl Completer for () {
    fn complete(&self, _: &str, _: usize) -> Option<&str> {
        None
    }
}

impl<T: Completer> Completer for &T {
    fn complete(&self, line: &str, idx: usize) -> Option<&str> {
        T::complete(self, line, idx)
    }
}

impl<T: Completer> Completer for &mut T {
    fn complete(&self, line: &str, idx: usize) -> Option<&str> {
        T::complete(self, line, idx)
    }
}

pub(crate) struct CompletionCycler<C> {
    completer: C,
    pre_completion_len: Option<usize>,
    idx: usize,
}

impl<C: Completer> CompletionCycler<C> {
    pub(crate) fn new(completer: C) -> Self {
        CompletionCycler {
            completer,
            pre_completion_len: None,
            idx: 0,
        }
    }

    pub(crate) fn complete<B: Buffer>(&mut self, line: &mut LineBuffer<B>) -> Result<(), ()> {
        let pre_completion_len = *self.pre_completion_len.get_or_insert(line.len());
        line.delete_after_char(pre_completion_len);
        if let Some(completion) = self.completer.complete(line.as_str(), self.idx) {
            self.idx += 1;
            line.insert_str(pre_completion_len, completion)
        } else {
            self.idx = 0;
            Ok(())
        }
    }

    pub(crate) fn reset(&mut self) {
        self.pre_completion_len = None;
        self.idx = 0;
    }
}
