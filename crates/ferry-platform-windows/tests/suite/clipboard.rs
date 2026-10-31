use ferry_platform_windows::{text_from_windows, text_to_windows};

fn utf16(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

#[test]
fn line_feeds_become_crlf_with_a_terminator() {
    let mut expected = utf16("一\r\n二\r\n");
    expected.push(0);
    assert_eq!(text_to_windows("一\n二\n"), expected);
}

#[test]
fn existing_crlf_is_not_doubled() {
    let mut expected = utf16("a\r\nb");
    expected.push(0);
    assert_eq!(text_to_windows("a\r\nb"), expected);
}

#[test]
fn reading_stops_at_the_terminator_and_normalizes_newlines() {
    let mut units = utf16("😀\r\nok");
    units.extend([0, 0x41, 0x42]);
    assert_eq!(text_from_windows(&units), "😀\nok");
}

#[test]
fn round_trip_keeps_the_text() {
    let text = "第一行\n第二行 😀\n";
    assert_eq!(text_from_windows(&text_to_windows(text)), text);
}
