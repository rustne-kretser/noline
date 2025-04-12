use crate::history::History;
use crate::input::{Action, CSI};
use crate::line_buffer::Buffer;
use crate::output::{Output, OutputAction};
use crate::terminal::Cursor;

use super::Line;
use OutputAction::*;

enum ResetState {
    New,
    GetSize,
    GetPosition,
    Done,
}

pub struct ResetHandle<'line, 'a, B: Buffer, H: History, I> {
    line: &'line mut Line<'a, B, H, I>,
    state: ResetState,
}

impl<'line, 'a, 'item, 'output, B, H, I> ResetHandle<'line, 'a, B, H, I>
where
    I: Iterator<Item = &'item str> + Clone + 'a,
    B: Buffer,
    H: History,
    'item: 'output,
{
    pub(super) fn new(line: &'line mut Line<'a, B, H, I>) -> Self {
        Self {
            line,
            state: ResetState::New,
        }
    }

    pub fn start(&mut self) -> Output<'_, B, I> {
        assert!(matches!(self.state, ResetState::New));
        self.state = ResetState::GetSize;

        self.line.generate_output(ProbeSize)
    }

    pub fn advance(&mut self, byte: u8) -> Option<Output<'_, B, I>> {
        let action = self.line.parser.advance(byte);

        match action {
            Action::ControlSequenceIntroducer(CSI::CPR(x, y)) => match self.state {
                ResetState::New => panic!("Invalid state"),
                ResetState::GetSize => {
                    self.line.terminal.resize(x, y);
                    self.state = ResetState::GetPosition;
                    Some(self.line.generate_output(ClearAndPrintPrompt))
                }
                ResetState::GetPosition => {
                    #[cfg(test)]
                    dbg!(x, y);
                    self.line.terminal.reset(Cursor::new(x - 1, y - 1));
                    self.state = ResetState::Done;
                    None
                }
                ResetState::Done => panic!("Invalid state"),
            },
            Action::Ignore => Some(self.line.generate_output(Nothing)),
            _ => None,
        }
    }
}
