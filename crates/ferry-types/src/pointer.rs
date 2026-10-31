//! 指针：鼠标按键与滚轮单位。

use std::num::NonZeroU8;

/// 滚轮转动一格对应的单位数。高精度滚动（触控板、无级滚轮）可以产生它的分数。
pub const WHEEL_NOTCH: i32 = 120;

/// 鼠标按键编号，从 1 开始：1–5 为标准按键，更大的编号是额外的侧键。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MouseButton(NonZeroU8);

impl MouseButton {
    pub const LEFT: Self = Self::from_const(1);
    pub const RIGHT: Self = Self::from_const(2);
    pub const MIDDLE: Self = Self::from_const(3);
    pub const BACK: Self = Self::from_const(4);
    pub const FORWARD: Self = Self::from_const(5);

    /// 编号为 0 时返回 `None`。
    pub const fn new(number: u8) -> Option<Self> {
        match NonZeroU8::new(number) {
            Some(n) => Some(Self(n)),
            None => None,
        }
    }

    pub const fn number(self) -> u8 {
        self.0.get()
    }

    const fn from_const(number: u8) -> Self {
        Self(NonZeroU8::new(number).expect("按键编号不能为 0"))
    }
}
