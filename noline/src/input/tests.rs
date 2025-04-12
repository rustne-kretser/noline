use super::state::State;
use super::ControlCharacter::*;
use super::*;
use crate::testlib::ToByteVec;
use crate::utf8::Utf8Char;
use std::vec::Vec;

fn input_sequence(parser: &mut Parser, seq: impl ToByteVec) -> Vec<Action> {
    seq.to_byte_vec()
        .into_iter()
        .map(|b| parser.advance(b))
        .collect()
}

#[test]
fn parser() {
    let mut parser = Parser::new();

    assert_eq!(parser.state, State::Ground);

    assert_eq!(parser.advance(b'a'), Action::Print(Utf8Char::from_str("a")));
    assert_eq!(parser.advance(0x7), Action::ControlCharacter(CtrlG));
    assert_eq!(parser.advance(0x3), Action::ControlCharacter(CtrlC));

    let actions = input_sequence(&mut parser, "æ");
    assert_eq!(
        actions,
        [Action::Ignore, Action::Print(Utf8Char::from_str("æ"))]
    );

    let mut actions = input_sequence(&mut parser, "\x1b[312;836R");
    assert_eq!(
        actions.pop().unwrap(),
        Action::ControlSequenceIntroducer(CSI::CPR(312, 836))
    );
    while let Some(action) = actions.pop() {
        assert_eq!(action, Action::Ignore);
    }

    let mut actions = input_sequence(&mut parser, "\x1b[R");
    assert_eq!(
        actions.pop().unwrap(),
        Action::ControlSequenceIntroducer(CSI::Invalid)
    );

    let mut actions = input_sequence(&mut parser, "\x1b[32R");
    assert_eq!(
        actions.pop().unwrap(),
        Action::ControlSequenceIntroducer(CSI::Invalid)
    );

    let mut actions = input_sequence(&mut parser, "\x1b[32;R");
    assert_eq!(
        actions.pop().unwrap(),
        Action::ControlSequenceIntroducer(CSI::Invalid)
    );

    let mut actions = input_sequence(&mut parser, "\x1b[A");

    assert_eq!(
        actions.pop().unwrap(),
        Action::ControlSequenceIntroducer(CSI::CUU(1))
    );

    let mut actions = input_sequence(&mut parser, "\x1b[10B");

    assert_eq!(
        actions.pop().unwrap(),
        Action::ControlSequenceIntroducer(CSI::CUD(10))
    );

    let mut actions = input_sequence(&mut parser, "\x1b[H");

    assert_eq!(
        actions.pop().unwrap(),
        Action::ControlSequenceIntroducer(CSI::CUP(1, 1))
    );

    let mut actions = input_sequence(&mut parser, "\x1b[2;5H");

    assert_eq!(
        actions.pop().unwrap(),
        Action::ControlSequenceIntroducer(CSI::CUP(2, 5))
    );

    let mut actions = input_sequence(&mut parser, "\x1b[;5H");

    assert_eq!(
        actions.pop().unwrap(),
        Action::ControlSequenceIntroducer(CSI::CUP(1, 5))
    );

    let mut actions = input_sequence(&mut parser, "\x1b[17;H");

    assert_eq!(
        actions.pop().unwrap(),
        Action::ControlSequenceIntroducer(CSI::CUP(17, 1))
    );

    let mut actions = input_sequence(&mut parser, "\x1b[;H");

    assert_eq!(
        actions.pop().unwrap(),
        Action::ControlSequenceIntroducer(CSI::CUP(1, 1))
    );

    let mut actions = input_sequence(&mut parser, "\x1b[;10H");

    assert_eq!(
        actions.pop().unwrap(),
        Action::ControlSequenceIntroducer(CSI::CUP(1, 10))
    );
}
