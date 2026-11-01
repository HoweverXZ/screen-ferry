//! 剪贴板：`Clipboard` 的 macOS 实现，基于 NSPasteboard。
//!
//! 读写纯文本、HTML、RTF 和图片。协议里的图片是 PNG：剪贴板里只有 TIFF 时（预览、Safari 复制的图片）
//! 转成 PNG 再交出去；写入时只放 PNG，现在的程序都认它。
//!
//! 在访达里复制文件时，剪贴板里还有一张文件图标，那不是用户想复制的图片，这时不读图片。
//!
//! NSPasteboard 可以在任意线程使用。每次调用都包在自动释放池里，
//! 因为 tokio 的工作线程没有现成的池，系统返回的临时对象会一直留在内存里。

use ferry_platform::{Clipboard, Error, Result};
use ferry_types::{ClipboardContent, ClipboardFormat};
use objc2::rc::autoreleasepool;
use objc2_app_kit::{
    NSBitmapImageFileType, NSBitmapImageRep, NSPasteboard, NSPasteboardType,
    NSPasteboardTypeFileURL, NSPasteboardTypeHTML, NSPasteboardTypePNG, NSPasteboardTypeRTF,
    NSPasteboardTypeString, NSPasteboardTypeTIFF,
};
use objc2_foundation::{NSData, NSDictionary, NSString};

/// 本机能读写的格式。
const FORMATS: &[ClipboardFormat] = &[
    ClipboardFormat::TEXT,
    ClipboardFormat::HTML,
    ClipboardFormat::RTF,
    ClipboardFormat::PNG,
];

/// 系统剪贴板。
#[derive(Clone, Copy, Debug, Default)]
pub struct MacClipboard;

impl MacClipboard {
    pub fn new() -> Self {
        Self
    }
}

/// AppKit 导出的剪贴板类型常量。
fn kind(format: ClipboardFormat) -> Option<&'static NSPasteboardType> {
    // SAFETY: AppKit 导出的常量字符串，进程存续期间一直有效。
    unsafe {
        Some(match format {
            ClipboardFormat::TEXT => NSPasteboardTypeString,
            ClipboardFormat::HTML => NSPasteboardTypeHTML,
            ClipboardFormat::RTF => NSPasteboardTypeRTF,
            ClipboardFormat::PNG => NSPasteboardTypePNG,
            _ => return None,
        })
    }
}

impl Clipboard for MacClipboard {
    fn formats(&self) -> &'static [ClipboardFormat] {
        FORMATS
    }

    fn change_count(&mut self) -> u64 {
        autoreleasepool(|_| NSPasteboard::generalPasteboard().changeCount() as u64)
    }

    fn read(&mut self) -> Result<ClipboardContent> {
        autoreleasepool(|_| {
            let board = NSPasteboard::generalPasteboard();
            let mut content = ClipboardContent::new();
            if let Some(text) = kind(ClipboardFormat::TEXT).and_then(|t| board.stringForType(t)) {
                content.insert(ClipboardFormat::TEXT, text.to_string().into_bytes());
            }
            if let Some(html) = kind(ClipboardFormat::HTML).and_then(|t| board.dataForType(t)) {
                content.insert(ClipboardFormat::HTML, utf8_html(&html.to_vec()));
            }
            if let Some(rtf) = kind(ClipboardFormat::RTF).and_then(|t| board.dataForType(t)) {
                content.insert(ClipboardFormat::RTF, rtf.to_vec());
            }
            if !has_files(&board)
                && let Some(png) = read_png(&board)
            {
                content.insert(ClipboardFormat::PNG, png);
            }
            Ok(content)
        })
    }

    fn write(&mut self, content: &ClipboardContent) -> Result<u64> {
        autoreleasepool(|_| {
            let board = NSPasteboard::generalPasteboard();
            board.clearContents();
            for (format, data) in content.iter() {
                let Some(kind) = kind(format) else {
                    continue;
                };
                let written = if format == ClipboardFormat::TEXT {
                    let text = String::from_utf8_lossy(data);
                    board.setString_forType(&NSString::from_str(&text), kind)
                } else {
                    board.setData_forType(Some(&NSData::with_bytes(data)), kind)
                };
                if !written {
                    return Err(Error::Os {
                        call: "NSPasteboard setData:forType:",
                        detail: "系统拒绝写入".into(),
                    });
                }
            }
            Ok(board.changeCount() as u64)
        })
    }
}

/// 剪贴板里是不是复制的文件（访达里复制文件时还会带上一张文件图标）。
fn has_files(board: &NSPasteboard) -> bool {
    // SAFETY: AppKit 导出的常量字符串，进程存续期间一直有效。
    let file_url = unsafe { NSPasteboardTypeFileURL };
    board
        .types()
        .is_some_and(|types| types.containsObject(file_url))
}

/// 剪贴板里的图片，转成 PNG。有 PNG 就直接用，只有 TIFF 时转换。
fn read_png(board: &NSPasteboard) -> Option<Vec<u8>> {
    if let Some(png) = kind(ClipboardFormat::PNG).and_then(|t| board.dataForType(t)) {
        return Some(png.to_vec());
    }
    // SAFETY: AppKit 导出的常量字符串，进程存续期间一直有效。
    let tiff = board.dataForType(unsafe { NSPasteboardTypeTIFF })?;
    let image = NSBitmapImageRep::imageRepWithData(&tiff)?;
    let properties = NSDictionary::new();
    // SAFETY: 不传任何属性，空字典的类型无关紧要。
    let png = unsafe {
        image.representationUsingType_properties(NSBitmapImageFileType::PNG, &properties)
    }?;
    Some(png.to_vec())
}

/// 剪贴板里的 HTML 一般是 UTF-8；个别程序放的是带字节序标记的 UTF-16，统一转成 UTF-8。
fn utf8_html(data: &[u8]) -> Vec<u8> {
    let utf16 = |bytes: &[u8], little: bool| {
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|p| {
                if little {
                    u16::from_le_bytes([p[0], p[1]])
                } else {
                    u16::from_be_bytes([p[0], p[1]])
                }
            })
            .collect();
        String::from_utf16_lossy(&units).into_bytes()
    };
    match data {
        [0xFF, 0xFE, rest @ ..] => utf16(rest, true),
        [0xFE, 0xFF, rest @ ..] => utf16(rest, false),
        [0xEF, 0xBB, 0xBF, rest @ ..] => rest.to_vec(),
        _ => data.to_vec(),
    }
}
