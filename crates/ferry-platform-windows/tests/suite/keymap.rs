use std::collections::HashMap;

use ferry_platform_windows::{WinKey, hid_from_windows, win_key};
use ferry_types::KeyCode;

fn scan(code: u16, extended: bool) -> Option<WinKey> {
    Some(WinKey::Scan { code, extended })
}

#[test]
fn common_keys() {
    let cases = [
        (0x04, scan(0x1E, false)), // A
        (0x1D, scan(0x2C, false)), // Z
        (0x1E, scan(0x02, false)), // 1
        (0x27, scan(0x0B, false)), // 0
        (0x28, scan(0x1C, false)), // Enter
        (0x3A, scan(0x3B, false)), // F1
        (0x43, scan(0x44, false)), // F10
        (0x45, scan(0x58, false)), // F12
        (0x4F, scan(0x4D, true)),  // →
        (0x58, scan(0x1C, true)),  // 小键盘 Enter
        (0x68, scan(0x64, false)), // F13
        (0xE3, scan(0x5B, true)),  // 左 Win
        (0xE6, scan(0x38, true)),  // 右 Alt
    ];
    for (usage, expected) in cases {
        assert_eq!(
            win_key(KeyCode::keyboard(usage)),
            expected,
            "HID {usage:#04x}"
        );
    }
}

#[test]
fn media_keys_use_virtual_keys_with_their_e0_scan_codes() {
    let media = |vk, scan| Some(WinKey::Media { vk, scan });
    let cases = [
        (KeyCode::VOLUME_UP, media(0xAF, 0x30)),
        (KeyCode::VOLUME_DOWN, media(0xAE, 0x2E)),
        (KeyCode::MUTE, media(0xAD, 0x20)),
        (KeyCode::PLAY_PAUSE, media(0xB3, 0x22)),
        (KeyCode::NEXT_TRACK, media(0xB0, 0x19)),
        (KeyCode::PREVIOUS_TRACK, media(0xB1, 0x10)),
        (KeyCode::STOP, media(0xB2, 0x24)),
    ];
    for (code, expected) in cases {
        assert_eq!(win_key(code), expected, "{code:?}");
    }
}

#[test]
fn unknown_pages_and_usages_are_unsupported() {
    assert_eq!(win_key(KeyCode::consumer(0x6F)), None, "亮度");
    assert_eq!(win_key(KeyCode::new(0x01, 0x81)), None, "通用桌面页");
}

#[test]
fn no_two_keys_share_a_windows_key() {
    let mut seen = HashMap::new();
    for usage in 0..=0xFF_u16 {
        // ISO 的 # 与美式 \ 在同一个物理位置，共用扫描码。
        if usage == 0x32 {
            continue;
        }
        if let Some(key) = win_key(KeyCode::keyboard(usage))
            && let Some(other) = seen.insert(key, usage)
        {
            panic!("HID {other:#04x} 与 {usage:#04x} 都映射到 {key:?}");
        }
    }
    assert!(seen.len() > 100, "实际映射了 {} 个键", seen.len());
}

#[test]
fn scan_codes_map_back_to_a_hid_key_on_the_same_spot() {
    for usage in 0..=0xFF {
        let Some(WinKey::Scan { code, extended }) = win_key(KeyCode::keyboard(usage)) else {
            continue;
        };
        let back = hid_from_windows(0, code, extended).expect("扫描码认得出来");
        // 共用一个扫描码的键（\ 与 ISO 的 #）取编号小的那个，注入时还是同一个位置。
        assert_eq!(
            win_key(back),
            Some(WinKey::Scan { code, extended }),
            "HID {usage:#04x}"
        );
    }
}

#[test]
fn virtual_keys_tell_apart_keys_that_scan_codes_cannot() {
    let cases = [
        // Pause 和 Num Lock 的扫描码都是 0x45。
        ((0x13, 0x45, false), KeyCode::keyboard(0x48)),
        ((0x90, 0x45, true), KeyCode::NUM_LOCK),
        // 按着 Alt 时 Print Screen 的扫描码变成 0x54。
        ((0x2C, 0x54, false), KeyCode::keyboard(0x46)),
        // 右 Shift 有没有扩展位都一样。
        ((0xA1, 0x36, false), KeyCode::RIGHT_SHIFT),
        ((0xA1, 0x36, true), KeyCode::RIGHT_SHIFT),
        ((0x5B, 0x5B, true), KeyCode::LEFT_GUI),
        ((0xA5, 0x38, true), KeyCode::RIGHT_ALT),
        // 媒体键报成消费者页，和 Mac 的媒体键一致。
        ((0xAF, 0x30, true), KeyCode::VOLUME_UP),
        ((0xB3, 0x22, true), KeyCode::PLAY_PAUSE),
    ];
    for ((vk, scan, extended), expected) in cases {
        assert_eq!(
            hid_from_windows(vk, scan, extended),
            Some(expected),
            "vk {vk:#04x} scan {scan:#04x}"
        );
    }
}

#[test]
fn synthesized_keys_are_not_recognized() {
    // AltGr 附带的那个左 Ctrl：扫描码 0x21D。
    assert_eq!(hid_from_windows(0xA2, 0x21D, false), None);
    // 输入法直接送进来的字符（VK_PACKET），没有扫描码。
    assert_eq!(hid_from_windows(0xE7, 0, false), None);
}
