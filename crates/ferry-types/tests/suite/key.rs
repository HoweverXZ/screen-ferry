use ferry_types::KeyCode;

#[test]
fn splits_page_and_usage() {
    let volume_up = KeyCode::new(KeyCode::CONSUMER_PAGE, 0xE9);
    assert_eq!(volume_up.page(), 0x0C);
    assert_eq!(volume_up.usage(), 0xE9);
    assert_eq!(volume_up.raw(), 0x000C_00E9);
    assert_eq!(KeyCode::from_raw(volume_up.raw()), volume_up);
}

#[test]
fn recognizes_exactly_eight_modifiers() {
    let modifiers = [
        KeyCode::LEFT_CTRL,
        KeyCode::LEFT_SHIFT,
        KeyCode::LEFT_ALT,
        KeyCode::LEFT_GUI,
        KeyCode::RIGHT_CTRL,
        KeyCode::RIGHT_SHIFT,
        KeyCode::RIGHT_ALT,
        KeyCode::RIGHT_GUI,
    ];
    assert!(modifiers.iter().all(|k| k.is_modifier()));

    let letter_a = KeyCode::keyboard(0x04);
    assert!(!letter_a.is_modifier());
    assert!(!KeyCode::CAPS_LOCK.is_modifier());
    assert!(!KeyCode::new(KeyCode::CONSUMER_PAGE, 0xE0).is_modifier());
}
