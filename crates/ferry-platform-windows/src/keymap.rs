//! 按键映射：USB HID 键码 → Windows 扫描码（Set 1）。
//!
//! 注入时用扫描码而不是虚拟键码：扫描码对应物理位置，不受键盘布局和输入法影响，
//! 中文、日文输入法都能正常处理。

use ferry_types::KeyCode;

/// 一个 Windows 按键的注入方式。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WinKey {
    /// 扫描码；`extended` 为真时带 E0 前缀（方向键、右侧修饰键等）。
    Scan { code: u16, extended: bool },
    /// 虚拟键码。只用于没有普通扫描码的键（Pause 的扫描码是 E1 序列）。
    Virtual(u16),
    /// 媒体键：虚拟键码加上它的 E0 扫描码。系统按虚拟键码处理，扫描码让底层钩子也能认出来。
    Media { vk: u16, scan: u16 },
}

/// `VK_PAUSE`
const VK_PAUSE: u16 = 0x13;

/// 媒体键：HID 消费者页用法 → （虚拟键码，E0 扫描码）。
fn media(code: KeyCode) -> Option<WinKey> {
    let (vk, scan) = match code {
        KeyCode::MUTE => (0xAD, 0x20),           // VK_VOLUME_MUTE
        KeyCode::VOLUME_DOWN => (0xAE, 0x2E),    // VK_VOLUME_DOWN
        KeyCode::VOLUME_UP => (0xAF, 0x30),      // VK_VOLUME_UP
        KeyCode::NEXT_TRACK => (0xB0, 0x19),     // VK_MEDIA_NEXT_TRACK
        KeyCode::PREVIOUS_TRACK => (0xB1, 0x10), // VK_MEDIA_PREV_TRACK
        KeyCode::STOP => (0xB2, 0x24),           // VK_MEDIA_STOP
        KeyCode::PLAY_PAUSE => (0xB3, 0x22),     // VK_MEDIA_PLAY_PAUSE
        _ => return None,
    };
    Some(WinKey::Media { vk, scan })
}

/// HID 键码对应的 Windows 按键。没有对应的键返回 `None`。
pub fn win_key(code: KeyCode) -> Option<WinKey> {
    if code.page() == KeyCode::CONSUMER_PAGE {
        return media(code);
    }
    if code.page() != KeyCode::KEYBOARD_PAGE {
        return None;
    }
    let scan = |code| {
        Some(WinKey::Scan {
            code,
            extended: false,
        })
    };
    let ext = |code| {
        Some(WinKey::Scan {
            code,
            extended: true,
        })
    };

    let usage = code.usage();
    match usage {
        0x04 => scan(0x1E),                       // A
        0x05 => scan(0x30),                       // B
        0x06 => scan(0x2E),                       // C
        0x07 => scan(0x20),                       // D
        0x08 => scan(0x12),                       // E
        0x09 => scan(0x21),                       // F
        0x0A => scan(0x22),                       // G
        0x0B => scan(0x23),                       // H
        0x0C => scan(0x17),                       // I
        0x0D => scan(0x24),                       // J
        0x0E => scan(0x25),                       // K
        0x0F => scan(0x26),                       // L
        0x10 => scan(0x32),                       // M
        0x11 => scan(0x31),                       // N
        0x12 => scan(0x18),                       // O
        0x13 => scan(0x19),                       // P
        0x14 => scan(0x10),                       // Q
        0x15 => scan(0x13),                       // R
        0x16 => scan(0x1F),                       // S
        0x17 => scan(0x14),                       // T
        0x18 => scan(0x16),                       // U
        0x19 => scan(0x2F),                       // V
        0x1A => scan(0x11),                       // W
        0x1B => scan(0x2D),                       // X
        0x1C => scan(0x15),                       // Y
        0x1D => scan(0x2C),                       // Z
        0x1E..=0x26 => scan(usage - 0x1E + 0x02), // 1–9
        0x27 => scan(0x0B),                       // 0
        0x28 => scan(0x1C),                       // Enter
        0x29 => scan(0x01),                       // Esc
        0x2A => scan(0x0E),                       // 退格
        0x2B => scan(0x0F),                       // Tab
        0x2C => scan(0x39),                       // 空格
        0x2D => scan(0x0C),                       // -
        0x2E => scan(0x0D),                       // =
        0x2F => scan(0x1A),                       // [
        0x30 => scan(0x1B),                       // ]
        0x31 => scan(0x2B),                       // \
        0x32 => scan(0x2B),                       // ISO 键盘的 #，与 \ 同一个位置
        0x33 => scan(0x27),                       // ;
        0x34 => scan(0x28),                       // '
        0x35 => scan(0x29),                       // `
        0x36 => scan(0x33),                       // ,
        0x37 => scan(0x34),                       // .
        0x38 => scan(0x35),                       // /
        0x39 => scan(0x3A),                       // Caps Lock
        0x3A..=0x43 => scan(usage - 0x3A + 0x3B), // F1–F10
        0x44 => scan(0x57),                       // F11
        0x45 => scan(0x58),                       // F12
        0x46 => ext(0x37),                        // Print Screen
        0x47 => scan(0x46),                       // Scroll Lock
        0x48 => Some(WinKey::Virtual(VK_PAUSE)),  // Pause
        0x49 => ext(0x52),                        // Insert
        0x4A => ext(0x47),                        // Home
        0x4B => ext(0x49),                        // Page Up
        0x4C => ext(0x53),                        // Delete
        0x4D => ext(0x4F),                        // End
        0x4E => ext(0x51),                        // Page Down
        0x4F => ext(0x4D),                        // →
        0x50 => ext(0x4B),                        // ←
        0x51 => ext(0x50),                        // ↓
        0x52 => ext(0x48),                        // ↑
        0x53 => ext(0x45),                        // Num Lock（Windows 视为扩展键）
        0x54 => ext(0x35),                        // 小键盘 /
        0x55 => scan(0x37),                       // 小键盘 *
        0x56 => scan(0x4A),                       // 小键盘 -
        0x57 => scan(0x4E),                       // 小键盘 +
        0x58 => ext(0x1C),                        // 小键盘 Enter
        0x59 => scan(0x4F),                       // 小键盘 1
        0x5A => scan(0x50),                       // 小键盘 2
        0x5B => scan(0x51),                       // 小键盘 3
        0x5C => scan(0x4B),                       // 小键盘 4
        0x5D => scan(0x4C),                       // 小键盘 5
        0x5E => scan(0x4D),                       // 小键盘 6
        0x5F => scan(0x47),                       // 小键盘 7
        0x60 => scan(0x48),                       // 小键盘 8
        0x61 => scan(0x49),                       // 小键盘 9
        0x62 => scan(0x52),                       // 小键盘 0
        0x63 => scan(0x53),                       // 小键盘 .
        0x64 => scan(0x56),                       // ISO 键盘 Z 左边的键
        0x65 => ext(0x5D),                        // 菜单键
        0x67 => scan(0x59),                       // 小键盘 =
        0x68..=0x6F => scan(usage - 0x68 + 0x64), // F13–F20
        0x70 => scan(0x6C),                       // F21
        0x71 => scan(0x6D),                       // F22
        0x72 => scan(0x6E),                       // F23
        0x73 => scan(0x76),                       // F24
        0x7F => ext(0x20),                        // 静音
        0x80 => ext(0x30),                        // 音量加
        0x81 => ext(0x2E),                        // 音量减
        0x85 => scan(0x7E),                       // 小键盘 ,
        0x87 => scan(0x73),                       // JIS ろ
        0x88 => scan(0x70),                       // JIS かな
        0x89 => scan(0x7D),                       // JIS ¥
        0x8A => scan(0x79),                       // 変換
        0x8B => scan(0x7B),                       // 無変換
        0xE0 => scan(0x1D),                       // 左 Ctrl
        0xE1 => scan(0x2A),                       // 左 Shift
        0xE2 => scan(0x38),                       // 左 Alt
        0xE3 => ext(0x5B),                        // 左 Win
        0xE4 => ext(0x1D),                        // 右 Ctrl
        0xE5 => scan(0x36),                       // 右 Shift
        0xE6 => ext(0x38),                        // 右 Alt
        0xE7 => ext(0x5C),                        // 右 Win
        _ => None,
    }
}
