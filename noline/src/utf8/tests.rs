use super::*;

#[test]
fn ascii() {
    let mut parser = Utf8Decoder::new();

    assert_eq!(
        parser.advance(b'a'),
        Utf8DecoderStatus::Done(Utf8Char::from_str("a"))
    );

    assert_eq!(parser.advance(b'a'), Utf8DecoderStatus::Error);
}

#[test]
fn twobyte() {
    let mut parser = Utf8Decoder::new();

    let bytes = "æ".as_bytes();

    assert_eq!(parser.advance(bytes[0]), Utf8DecoderStatus::Continuation);

    assert_eq!(
        parser.advance(bytes[1]),
        Utf8DecoderStatus::Done(Utf8Char::from_str("æ"))
    );

    assert_eq!(parser.advance(b'a'), Utf8DecoderStatus::Error);
}

#[test]
fn threebyte() {
    let mut parser = Utf8Decoder::new();

    let bytes = "€".as_bytes();

    assert_eq!(parser.advance(bytes[0]), Utf8DecoderStatus::Continuation);
    assert_eq!(parser.advance(bytes[1]), Utf8DecoderStatus::Continuation);

    assert_eq!(
        parser.advance(bytes[2]),
        Utf8DecoderStatus::Done(Utf8Char::from_str("€"))
    );

    assert_eq!(parser.advance(b'a'), Utf8DecoderStatus::Error);
}

#[test]
fn fourbyte() {
    let mut parser = Utf8Decoder::new();

    let symbol = "😂";

    let bytes = symbol.as_bytes();
    dbg!(bytes);

    assert_eq!(parser.advance(bytes[0]), Utf8DecoderStatus::Continuation);
    assert_eq!(parser.advance(bytes[1]), Utf8DecoderStatus::Continuation);
    assert_eq!(parser.advance(bytes[2]), Utf8DecoderStatus::Continuation);

    assert_eq!(
        parser.advance(bytes[3]),
        Utf8DecoderStatus::Done(Utf8Char::from_str(symbol))
    );

    assert_eq!(parser.advance(b'a'), Utf8DecoderStatus::Error);
}

#[test]
fn invalid_start() {
    let mut parser = Utf8Decoder::new();

    assert_eq!(parser.advance(0b10000000), Utf8DecoderStatus::Error);
}

#[test]
fn invalid_continuation() {
    let mut parser = Utf8Decoder::new();

    assert_eq!(parser.advance(0b11000000), Utf8DecoderStatus::Continuation);
    assert_eq!(parser.advance(0b00000000), Utf8DecoderStatus::Error);
}

#[test]
fn to_char() {
    assert_eq!(Utf8Char::from_str("€").as_char(), '€');
}
