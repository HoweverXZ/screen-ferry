//! 显示器。

use crate::geometry::Rect;

/// 显示器在所属设备内的编号。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DisplayId(pub u32);

/// 一块显示器。
///
/// `bounds` 使用所属设备自己的桌面坐标系：macOS 为点（point），Windows 为物理像素。
/// 发给某台设备的坐标，一律使用那台设备的坐标系。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Display {
    pub id: DisplayId,
    pub bounds: Rect,
    /// 一个逻辑单位等于多少个坐标单位，以百分数表示。
    ///
    /// Windows 的坐标是物理像素，这里就是系统缩放比例（例如 150）；
    /// macOS 的坐标本身就是逻辑单位（点），这里恒为 100。
    /// 两台设备之间的移动距离按 `对方 / 本机` 换算，手感就能保持一致。
    pub scale_percent: u16,
    pub primary: bool,
}
