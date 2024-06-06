use crate::line_buffer::{Buffer, LineBuffer};

/// A type that provides possible line completions
///
/// ```
/// use noline::complete::Completer;
///
/// static FRUIT_LIST: [&str; 14] = [
///     "Apple",
///     "Banana",
///     "Grape",
///     "Kiwi",
///     "Lemon",
///     "Lime",
///     "Mango",
///     "Melon",
///     "Nectarine",
///     "Orange",
///     "Peach",
///     "Pear",
///     "Pineapple",
///     "Plum",
/// ];
///
/// struct Fruit;
///
/// impl Completer for Fruit {
///     fn complete(&self, line: &str, n: usize) -> Option<&str> {
///         FRUIT_LIST.iter()
///             .filter(|candidate| candidate.starts_with(line))
///             .skip(n)
///             .next()
///             .map(|candidate| &candidate[line.len()..])
///     }
/// }
///
/// assert_eq!(Fruit.complete("Pe", 1), Some("ar"))
/// ```
pub trait Completer {
    /// Given `line` return the `n`'th possible continuation
    fn complete(&self, line: &str, n: usize) -> Option<&str>;
}

impl Completer for () {
    fn complete(&self, _: &str, _: usize) -> Option<&str> {
        None
    }
}

impl<T: Completer> Completer for &T {
    fn complete(&self, line: &str, n: usize) -> Option<&str> {
        T::complete(self, line, n)
    }
}

impl<T: Completer> Completer for &mut T {
    fn complete(&self, line: &str, n: usize) -> Option<&str> {
        T::complete(self, line, n)
    }
}

pub(crate) struct CompletionCycler<C> {
    completer: C,
    pre_completion_len: Option<usize>,
    n: usize,
}

impl<C: Completer> CompletionCycler<C> {
    pub(crate) fn new(completer: C) -> Self {
        CompletionCycler {
            completer,
            pre_completion_len: None,
            n: 0,
        }
    }

    pub(crate) fn complete<B: Buffer>(&mut self, line: &mut LineBuffer<B>) -> Result<(), ()> {
        let pre_completion_len = *self.pre_completion_len.get_or_insert(line.len());
        line.delete_after_char(pre_completion_len);
        if let Some(completion) = self.completer.complete(line.as_str(), self.n) {
            self.n += 1;
            line.insert_str(pre_completion_len, completion)
        } else {
            self.n = 0;
            Ok(())
        }
    }

    pub(crate) fn reset(&mut self) {
        self.pre_completion_len = None;
        self.n = 0;
    }
}
