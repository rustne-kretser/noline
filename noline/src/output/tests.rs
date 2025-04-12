use super::*;
use super::{
    move_cursor::MoveCursor, move_cursor_pos::MoveCursorToPosition, printable::Printable,
    step::Step,
};
use crate::{
    core::{Prompt, StrIter},
    line_buffer::{Buffer, LineBuffer},
    terminal::{Cursor, Position, Terminal},
};
use std::string::String;
use std::vec::Vec;

#[test]
fn uint_to_bytes() {
    fn to_string<const N: usize>(n: usize) -> String {
        let uint: UintToBytes<N> = UintToBytes::from_uint(n).unwrap();

        String::from_utf8(uint.as_bytes().to_vec()).unwrap()
    }

    assert_eq!(to_string::<4>(0), "0");

    assert_eq!(to_string::<4>(42), "42");

    assert_eq!(to_string::<4>(10), "10");

    assert_eq!(to_string::<4>(9999), "9999");
}

#[test]
fn move_cursor() {
    fn to_string(cm: MoveCursor) -> String {
        String::from_utf8(
            cm.flat_map(|item| {
                if let Some(bytes) = item.get_bytes() {
                    bytes.to_vec()
                } else {
                    vec![]
                }
            })
            .collect(),
        )
        .unwrap()
    }

    assert_eq!(
        to_string(MoveCursor::new(Cursor::new(42, 0), 0)),
        "\x1b[43;1H"
    );

    assert_eq!(
        to_string(MoveCursor::new(Cursor::new(0, 42), 0)),
        "\x1b[1;43H"
    );

    assert_eq!(
        to_string(MoveCursor::new(Cursor::new(42, 43), 0)),
        "\x1b[43;44H"
    );

    assert_eq!(
        to_string(MoveCursor::new(Cursor::new(0, 0), 0)),
        "\x1b[1;1H"
    );

    assert_eq!(
        to_string(MoveCursor::new(Cursor::new(0, 9), 0)),
        "\x1b[1;10H"
    );

    assert_eq!(
        to_string(MoveCursor::new(Cursor::new(0, 0), 1)),
        "\x1b[1S\x1b[1;1H"
    );

    assert_eq!(
        to_string(MoveCursor::new(Cursor::new(0, 0), -1)),
        "\x1b[1T\x1b[1;1H"
    );
}

#[test]
fn step() {
    fn to_string<'a>(mut step: Step<'a, StrIter<'a>>, terminal: &mut Terminal) -> String {
        let mut bytes = Vec::new();

        while let Some(item) = step.advance(terminal) {
            if let Some(slice) = item.get_bytes() {
                for b in slice {
                    bytes.push(*b);
                }
            }
        }

        String::from_utf8(bytes).unwrap()
    }

    let mut terminal = Terminal::new(4, 10, Cursor::new(0, 0));

    assert_eq!(
        to_string(
            Step::Print(Printable::from_str("01234567890123456789")),
            &mut terminal
        ),
        "0123456789\n\r0123456789\n\r"
    );

    assert_eq!(
        to_string(Step::Print(Printable::from_str("01234")), &mut terminal),
        "01234"
    );

    assert_eq!(
        to_string(
            Step::Print(Printable::from_str("5678901234567890")),
            &mut terminal
        ),
        "56789\n\r0123456789\n\r0"
    );

    assert_eq!(terminal.get_position(), Position::new(4, 1));

    assert_eq!(
        to_string(
            Step::Move(MoveCursorToPosition::new(Position::new(0, 3))),
            &mut terminal
        ),
        "\x1b[1T\x1b[1;4H"
    );

    assert_eq!(terminal.get_position(), Position::new(0, 3));

    assert_eq!(to_string(Step::Erase, &mut terminal), "\x1b[J");
    assert_eq!(to_string(Step::Newline, &mut terminal), "\n\r");
    assert_eq!(to_string(Step::Bell, &mut terminal), "\x07");
    assert_eq!(to_string(Step::Done, &mut terminal), "");
}

#[test]
fn byte_iterator() {
    fn to_string<B: Buffer>(output: Output<'_, B, StrIter>) -> String {
        String::from_utf8(
            output
                .into_iter()
                .flat_map(|item| {
                    if let Some(bytes) = item.get_bytes() {
                        bytes.to_vec()
                    } else {
                        vec![]
                    }
                })
                .collect(),
        )
        .unwrap()
    }

    let prompt: Prompt<StrIter> = "> ".into();
    let mut line_buffer = LineBuffer::new_unbounded();
    let mut terminal = Terminal::new(4, 10, Cursor::new(0, 0));

    let result = to_string(Output::new(
        &prompt,
        &line_buffer,
        &mut terminal,
        OutputAction::ClearAndPrintPrompt,
    ));

    assert_eq!(result, "\r\x1b[J> \x1b[6n");

    line_buffer.insert_str(0, "Hello, world!").unwrap();

    let result = to_string(Output::new(
        &prompt,
        &line_buffer,
        &mut terminal,
        OutputAction::PrintBufferAndMoveCursorForward,
    ));

    assert_eq!(result, "Hello, w\n\rorld!\x1b[1;4H");

    assert_eq!(terminal.get_cursor(), Cursor::new(0, 3));

    let result = to_string(Output::new(
        &prompt,
        &line_buffer,
        &mut terminal,
        OutputAction::MoveCursor(CursorMove::Start),
    ));

    assert_eq!(result, "\x1b[1;3H");
    assert_eq!(terminal.get_cursor(), Cursor::new(0, 2));
}

#[test]
fn split_utf8() {
    fn to_string<'a>(mut step: Step<'a, StrIter<'a>>, terminal: &mut Terminal) -> String {
        let mut bytes = Vec::new();

        while let Some(item) = step.advance(terminal) {
            if let Some(slice) = item.get_bytes() {
                for b in slice {
                    bytes.push(*b);
                }
            }
        }

        String::from_utf8(bytes).unwrap()
    }

    let mut terminal = Terminal::new(4, 10, Cursor::new(0, 0));

    assert_eq!(
        to_string(
            Step::Print(Printable::from_str("aadfåpadfåaåfåaadåappaåadå")),
            &mut terminal
        ),
        "aadfåpadfå\n\raåfåaadåap\n\rpaåadå"
    );
}
