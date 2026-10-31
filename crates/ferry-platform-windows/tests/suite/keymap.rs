use std::collections::HashMap;

use ferry_platform_windows::{WinKey, win_key};
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
fn only_the_keyboard_page_is_supported() {
    assert_eq!(win_key(KeyCode::new(KeyCode::CONSUMER_PAGE, 0xE9)), None);
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
