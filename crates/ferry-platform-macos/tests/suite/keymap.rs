use std::collections::HashMap;

use ferry_platform_macos::{hid_code, mac_keycode, media_key, nx_key};
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

#[test]
fn media_keys_map_to_the_consumer_page() {
    let cases = [
        (0, KeyCode::VOLUME_UP),
        (1, KeyCode::VOLUME_DOWN),
        (7, KeyCode::MUTE),
        (16, KeyCode::PLAY_PAUSE),
        (17, KeyCode::NEXT_TRACK),
        (18, KeyCode::PREVIOUS_TRACK),
    ];
    for (nx, hid) in cases {
        assert_eq!(media_key(nx), Some(hid), "NX_KEYTYPE {nx}");
        assert_eq!(hid.page(), KeyCode::CONSUMER_PAGE);
    }
}

#[test]
fn keys_the_other_side_lacks_stay_on_the_mac() {
    // 亮度、Caps Lock、键盘背光：对方没有对应的键，留给本机。
    for nx in [2, 3, 4, 21, 22, 23] {
        assert_eq!(media_key(nx), None, "NX_KEYTYPE {nx}");
    }
}

#[test]
fn every_mapped_key_maps_back_to_the_same_mac_key() {
    for mac in 0..=0x7F_u16 {
        if let Some(hid) = hid_code(mac) {
            assert_eq!(mac_keycode(hid), Some(mac), "{mac:#04x} → {hid:?}");
        }
    }
}

#[test]
fn keys_a_mac_keyboard_lacks_have_no_mac_key() {
    // Print Screen、Scroll Lock、Pause，以及消费者页的媒体键（它们走 nx_key）。
    for hid in [
        KeyCode::keyboard(0x46),
        KeyCode::SCROLL_LOCK,
        KeyCode::keyboard(0x48),
        KeyCode::VOLUME_UP,
    ] {
        assert_eq!(mac_keycode(hid), None, "{hid:?}");
    }
}

#[test]
fn media_keys_map_back_to_their_nx_key_types() {
    for nx in 0..32 {
        if let Some(hid) = media_key(nx) {
            assert_eq!(nx_key(hid), Some(nx), "NX_KEYTYPE {nx}");
        }
    }
    // Windows 键盘的音量键报成键盘页上的键，在 Mac 上同样要当媒体键发出才调得了音量。
    assert_eq!(nx_key(KeyCode::keyboard(0x80)), Some(0));
    assert_eq!(nx_key(KeyCode::keyboard(0x81)), Some(1));
    assert_eq!(nx_key(KeyCode::keyboard(0x7F)), Some(7));
    assert_eq!(nx_key(KeyCode::STOP), None);
}
