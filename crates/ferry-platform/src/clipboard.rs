//! 剪贴板：定义 `Clipboard` trait。

use ferry_types::{ClipboardContent, ClipboardFormat};

use crate::error::Result;

/// 读写本机剪贴板。
///
/// 系统剪贴板没有统一的变化通知，所以由调用方轮询 [`change_count`](Self::change_count)：
/// 它很便宜，内容每变一次它就变一次。
pub trait Clipboard: Send + 'static {
    /// 本机能读写的格式。
    fn formats(&self) -> &'static [ClipboardFormat];

    /// 剪贴板的变化计数。只比较是否相等，不要求递增。
    fn change_count(&mut self) -> u64;

    /// 读取当前内容中本机支持的格式。剪贴板里没有这些格式时返回空内容。
    fn read(&mut self) -> Result<ClipboardContent>;

    /// 用 `content` 替换剪贴板，返回写入之后的变化计数，调用方据此识别出这是自己写的。
    fn write(&mut self, content: &ClipboardContent) -> Result<u64>;
}
