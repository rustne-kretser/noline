use crate::utf8::Utf8Decoder;

#[derive(Debug, Eq, PartialEq)]
pub(super) enum State {
    Ground,
    Utf8Sequence(Option<Utf8Decoder>),
    EscapeSequence,
    CSIStart,
    CSIArg1(Option<usize>),
    CSIArg2(Option<usize>, Option<usize>),
}
