//! 剪贴板数据格式。

/// 剪贴板数据格式。以后可能新增格式，所以用编号而不是封闭枚举。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ClipboardFormat(pub u8);

impl ClipboardFormat {
    /// UTF-8 纯文本。
    pub const TEXT: Self = Self(1);
    /// UTF-8 HTML 片段。
    pub const HTML: Self = Self(2);
    /// RTF 富文本。
    pub const RTF: Self = Self(3);
    /// PNG 图片。
    pub const PNG: Self = Self(4);
}
