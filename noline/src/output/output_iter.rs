use super::{step::Step, OutputItem};
use crate::terminal::Terminal;
use core::marker::PhantomData;

pub struct OutputIter<'a, 'item, I> {
    pub(super) terminal: &'a mut Terminal,
    pub(super) steps: [Option<Step<'a, I>>; 4],
    pub(super) pos: usize,
    pub(super) _marker: PhantomData<&'item ()>,
}

impl<'a, 'item, I> Iterator for OutputIter<'a, 'item, I>
where
    I: Iterator<Item = &'item str>,
    'item: 'a,
{
    type Item = OutputItem<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(step) = self.steps.get_mut(self.pos) {
                if let Some(step) = step.as_mut() {
                    if let Some(item) = step.advance(self.terminal) {
                        break Some(item);
                    } else {
                        self.pos += 1;
                    }
                } else {
                    break None;
                }
            } else {
                break None;
            }
        }
    }
}
