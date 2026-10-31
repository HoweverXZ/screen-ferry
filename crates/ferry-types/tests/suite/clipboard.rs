use ferry_types::{ClipboardContent, ClipboardFormat};

#[test]
fn text_content_holds_one_format() {
    let content = ClipboardContent::text("你好");
    assert_eq!(content.as_text(), Some("你好"));
    assert_eq!(content.total_len(), "你好".len());
    assert_eq!(content.iter().count(), 1);
}

#[test]
fn inserting_the_same_format_replaces_it() {
    let mut content = ClipboardContent::text("旧");
    content.insert(ClipboardFormat::TEXT, b"new".to_vec());
    content.insert(ClipboardFormat::HTML, b"<b>new</b>".to_vec());

    let formats: Vec<_> = content.iter().map(|(f, _)| f).collect();
    assert_eq!(formats, [ClipboardFormat::TEXT, ClipboardFormat::HTML]);
    assert_eq!(content.as_text(), Some("new"));
}

#[test]
fn invalid_utf8_is_not_text() {
    let mut content = ClipboardContent::new();
    content.insert(ClipboardFormat::TEXT, vec![0xFF, 0xFE]);
    assert_eq!(content.as_text(), None);
    assert!(!content.is_empty());
}
