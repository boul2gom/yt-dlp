use media_seek::Error;

#[test]
fn error_unsupported_format_display() {
    let err = Error::UnsupportedFormat;
    let msg = format!("{}", err);
    assert!(msg.contains("Unsupported"));
}

#[test]
fn error_parse_failed_display() {
    let err = Error::ParseFailed {
        reason: "truncated header".to_string(),
    };
    let msg = format!("{}", err);
    assert!(msg.contains("truncated header"));
}

#[test]
fn error_fetch_failed_display() {
    let source = std::io::Error::new(std::io::ErrorKind::TimedOut, "connection timed out");
    let err = Error::FetchFailed(Box::new(source));
    let msg = format!("{}", err);
    assert!(msg.contains("Range fetch failed"));
}

#[test]
fn error_is_debug() {
    let err = Error::UnsupportedFormat;
    let debug = format!("{:?}", err);
    assert!(debug.contains("UnsupportedFormat"));
}
