//! 按键：USB HID 键码、按键动作、锁定键状态。

use bitflags::bitflags;

/// 物理键，用 USB HID 扩展用法码表示：高 16 位是用法页，低 16 位是用法 ID。
///
/// 常规按键位于键盘页（`0x07`），媒体键位于消费者页（`0x0C`）。
/// 修饰键也是普通按键（`0xE0`–`0xE7`），按下与松开各自单独传输。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct KeyCode(u32);

impl KeyCode {
    /// 键盘页。
    pub const KEYBOARD_PAGE: u16 = 0x07;
    /// 消费者页：音量、播放控制等媒体键。
    pub const CONSUMER_PAGE: u16 = 0x0C;

    pub const ESCAPE: Self = Self::keyboard(0x29);
    pub const CAPS_LOCK: Self = Self::keyboard(0x39);
    pub const SCROLL_LOCK: Self = Self::keyboard(0x47);
    pub const NUM_LOCK: Self = Self::keyboard(0x53);
    pub const LEFT_CTRL: Self = Self::keyboard(0xE0);
    pub const LEFT_SHIFT: Self = Self::keyboard(0xE1);
    pub const LEFT_ALT: Self = Self::keyboard(0xE2);
    /// 左 Windows 键 / 左 Command 键。
    pub const LEFT_GUI: Self = Self::keyboard(0xE3);
    pub const RIGHT_CTRL: Self = Self::keyboard(0xE4);
    pub const RIGHT_SHIFT: Self = Self::keyboard(0xE5);
    pub const RIGHT_ALT: Self = Self::keyboard(0xE6);
    /// 右 Windows 键 / 右 Command 键。
    pub const RIGHT_GUI: Self = Self::keyboard(0xE7);

    pub const fn new(page: u16, usage: u16) -> Self {
        Self(((page as u32) << 16) | usage as u32)
    }

    /// 键盘页上的按键。
    pub const fn keyboard(usage: u16) -> Self {
        Self::new(Self::KEYBOARD_PAGE, usage)
    }

    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u32 {
        self.0
    }

    pub const fn page(self) -> u16 {
        (self.0 >> 16) as u16
    }

    pub const fn usage(self) -> u16 {
        self.0 as u16
    }

    /// 是否为 Ctrl、Shift、Alt、GUI 之一。
    pub const fn is_modifier(self) -> bool {
        self.page() == Self::KEYBOARD_PAGE && matches!(self.usage(), 0xE0..=0xE7)
    }
}

/// 按键动作。
///
/// 系统不会为注入的按键自动连发，所以连发由捕获端以 `Repeat` 显式传输。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyAction {
    Down,
    Up,
    Repeat,
}

bitflags! {
    /// 锁定键状态，进入屏幕时同步给对方。
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct LockKeys: u8 {
        const CAPS = 1 << 0;
        const NUM = 1 << 1;
        const SCROLL = 1 << 2;
    }
}
