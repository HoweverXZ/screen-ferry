//! 几何：桌面坐标系中的点与矩形，以及两台设备屏幕的相对位置。

use serde::{Deserialize, Serialize};

/// 桌面坐标系中的点。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// 桌面坐标系中的矩形，包含左上边界，不包含右下边界。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// 右边界（不含）。超出 `i32` 范围时取 `i32::MAX`。
    pub const fn right(self) -> i32 {
        self.x.saturating_add_unsigned(self.width)
    }

    /// 下边界（不含）。超出 `i32` 范围时取 `i32::MAX`。
    pub const fn bottom(self) -> i32 {
        self.y.saturating_add_unsigned(self.height)
    }

    pub const fn contains(self, p: Point) -> bool {
        p.x >= self.x && p.x < self.right() && p.y >= self.y && p.y < self.bottom()
    }
}

/// 远端设备位于本机的哪一侧。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

impl Side {
    /// 从对方看过来的方向：A 在 B 的左边，B 就在 A 的右边。
    pub fn opposite(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
            Self::Top => Self::Bottom,
            Self::Bottom => Self::Top,
        }
    }
}
