use media_seek::ByteRange;

#[test]
fn byte_range_equality() {
    let a = ByteRange { start: 0, end: 100 };
    let b = ByteRange { start: 0, end: 100 };
    assert_eq!(a, b);
}

#[test]
fn byte_range_inequality() {
    let a = ByteRange { start: 0, end: 100 };
    let b = ByteRange { start: 0, end: 200 };
    assert_ne!(a, b);
}

#[test]
fn byte_range_copy() {
    let a = ByteRange { start: 10, end: 200 };
    let b = a;
    assert_eq!(a.start, b.start);
    assert_eq!(a.end, b.end);
}

#[test]
fn byte_range_debug() {
    let range = ByteRange { start: 42, end: 99 };
    let debug = format!("{:?}", range);
    assert!(debug.contains("42"));
    assert!(debug.contains("99"));
}
