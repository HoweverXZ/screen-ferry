//! 剪贴板：`Clipboard` 的 macOS 实现，基于 NSPasteboard。
//!
//! NSPasteboard 可以在任意线程使用。每次调用都包在自动释放池里，
//! 因为 tokio 的工作线程没有现成的池，系统返回的临时对象会一直留在内存里。

use ferry_platform::{Clipboard, Error, Result};
use ferry_types::{ClipboardContent, ClipboardFormat};
use objc2::rc::autoreleasepool;
use objc2_app_kit::{NSPasteboard, NSPasteboardType, NSPasteboardTypeString};
use objc2_foundation::NSString;

/// 系统剪贴板。
#[derive(Clone, Copy, Debug, Default)]
pub struct MacClipboard;

impl MacClipboard {
    pub fn new() -> Self {
        Self
    }
}

fn text_type() -> &'static NSPasteboardType {
    // SAFETY: AppKit 导出的常量字符串，进程存续期间一直有效。
    unsafe { NSPasteboardTypeString }
}

impl Clipboard for MacClipboard {
    fn formats(&self) -> &'static [ClipboardFormat] {
        &[ClipboardFormat::TEXT]
    }

    fn change_count(&mut self) -> u64 {
        autoreleasepool(|_| NSPasteboard::generalPasteboard().changeCount() as u64)
    }

    fn read(&mut self) -> Result<ClipboardContent> {
        autoreleasepool(|_| {
            let text = NSPasteboard::generalPasteboard().stringForType(text_type());
            Ok(text.map_or_else(ClipboardContent::new, |s| {
                ClipboardContent::text(s.to_string())
            }))
        })
    }

    fn write(&mut self, content: &ClipboardContent) -> Result<u64> {
        autoreleasepool(|_| {
            let board = NSPasteboard::generalPasteboard();
            board.clearContents();
            if let Some(text) = content.as_text()
                && !board.setString_forType(&NSString::from_str(text), text_type())
            {
                return Err(Error::Os {
                    call: "NSPasteboard setString:forType:",
                    detail: "系统拒绝写入".into(),
                });
            }
            Ok(board.changeCount() as u64)
        })
    }
}
