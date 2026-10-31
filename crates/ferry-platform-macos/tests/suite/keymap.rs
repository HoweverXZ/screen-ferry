use std::collections::HashMap;

use ferry_platform_macos::hid_code;
use ferry_types::KeyCode;

#[test]
fn letters_and_digits_follow_hid_order() {
    // kVK_ANSI_A … kVK_ANSI_Z 不连续，按字母逐个核对。
    let letters = [
        0x00, 0x0B, 0x08, 0x02, 0x0E, 0x03, 0x05, 0x04, 0x22, 0x26, 0x28, 0x25, 0x2E, 0x2D, 0x1F,
        0x23, 0x0C, 0x0F, 0x01, 0x11, 0x20, 0x09, 0x0D, 0x07, 0x10, 0x06,
    ];
    for (i, mac) in letters.into_iter().enumerate() {
        assert_eq!(
            hid_code(mac),
            Some(KeyCode::keyboard(0x04 + i as u16)),
            "字母 {}",
            (b'A' + i as u8) as char
        );
    }
    let digits = [0x12, 0x13, 0x14, 0x15, 0x17, 0x16, 0x1A, 0x1C, 0x19, 0x1D];
    for (i, mac) in digits.into_iter().enumerate() {
        assert_eq!(
            hid_code(mac),
            Some(KeyCode::keyboard(0x1E + i as u16)),
            "数字 {}",
            (i + 1) % 10
        );
    }
}

#[test]
fn modifiers_map_to_the_right_side() {
    let cases = [
        (0x3B, KeyCode::LEFT_CTRL),
        (0x38, KeyCode::LEFT_SHIFT),
        (0x3A, KeyCode::LEFT_ALT),
        (0x37, KeyCode::LEFT_GUI),
        (0x3E, KeyCode::RIGHT_CTRL),
        (0x3C, KeyCode::RIGHT_SHIFT),
        (0x3D, KeyCode::RIGHT_ALT),
        (0x36, KeyCode::RIGHT_GUI),
        (0x39, KeyCode::CAPS_LOCK),
    ];
    for (mac, hid) in cases {
        assert_eq!(hid_code(mac), Some(hid), "{mac:#04x}");
    }
}

#[test]
fn no_two_keys_share_a_hid_code() {
    let mut seen = HashMap::new();
    for mac in 0..=0x7F_u16 {
        if let Some(hid) = hid_code(mac)
            && let Some(other) = seen.insert(hid, mac)
        {
            panic!("{other:#04x} 与 {mac:#04x} 都映射到 {hid:?}");
        }
    }
    assert!(seen.len() > 100, "实际映射了 {} 个键", seen.len());
}

#[test]
fn fn_key_has_no_hid_code() {
    assert_eq!(hid_code(0x3F), None);
}
