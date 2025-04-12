use super::*;
use crate::utf8::Utf8Char;

#[test]
fn slice_buffer() {
    let mut array = [0; 20];
    let mut buf = SliceBuffer::new(&mut array);

    for i in 0..20 {
        buf.insert_byte(i, 0x30);
    }

    buf.remove_byte(19);
}

fn insert_str<B: Buffer>(buf: &mut LineBuffer<B>, index: usize, s: &str) {
    buf.insert_str(index, s).unwrap();
}

fn test_line_buffer<B: Buffer>(buf: &mut LineBuffer<B>) {
    insert_str(buf, 0, "Hello, World!");

    assert_eq!(buf.as_str(), "Hello, World!");

    buf.delete(12);

    assert_eq!(buf.as_str(), "Hello, World");

    buf.delete(12);

    assert_eq!(buf.as_str(), "Hello, World");

    buf.delete(0);
    insert_str(buf, 0, "h");

    assert_eq!(buf.as_str(), "hello, World");

    buf.delete(2);
    insert_str(buf, 2, "L");

    assert_eq!(buf.as_str(), "heLlo, World");

    buf.delete(11);

    assert_eq!(buf.as_str(), "heLlo, Worl");

    buf.delete(5);

    assert_eq!(buf.as_str(), "heLlo Worl");

    for _ in 0..5 {
        buf.delete(5);
    }

    assert_eq!(buf.as_str(), "heLlo");

    insert_str(buf, 5, " æå");

    assert_eq!(buf.as_str(), "heLlo æå");

    insert_str(buf, 7, "ø");

    assert_eq!(buf.as_str(), "heLlo æøå");

    buf.delete(8);

    assert_eq!(buf.as_str(), "heLlo æø");

    buf.delete(7);

    assert_eq!(buf.as_str(), "heLlo æ");

    buf.delete_previous_word(7);

    assert_eq!(buf.as_str(), "heLlo ");

    buf.delete_previous_word(6);

    assert_eq!(buf.as_str(), "");

    insert_str(buf, 0, "word1 word2 word3");
    assert_eq!(buf.as_str(), "word1 word2 word3");
    buf.delete_previous_word(12);

    assert_eq!(buf.as_str(), "word1 word3");
}

#[test]
fn test_slice_line_buffer() {
    let mut array = [0; 80];
    let mut buf = LineBuffer::from_slice(&mut array);

    test_line_buffer(&mut buf);

    buf.delete_after_char(0);

    assert_eq!(buf.len(), 0);

    for i in 0..80 {
        assert!(buf.insert_utf8_char(i, Utf8Char::from_str("a")).is_ok());
    }

    assert!(buf.insert_utf8_char(80, Utf8Char::from_str("a")).is_err());
}

#[test]
fn test_alloc_line_buffer() {
    let mut buf = LineBuffer::new_unbounded();

    test_line_buffer(&mut buf);

    buf.delete_after_char(0);

    for i in 0..1000 {
        assert!(buf.insert_utf8_char(i, Utf8Char::from_str("a")).is_ok());
    }
}
