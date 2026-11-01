//! 剪贴板：`Clipboard` 的 Windows 实现，基于 Win32 剪贴板 API。
//!
//! - 文本以 `CF_UNICODETEXT`（UTF-16）读写，系统会自动为旧程序生成其他文本格式。
//!   协议中的文本换行是 `\n`，Windows 习惯 `\r\n`，进出时各转换一次。
//! - HTML 用 `HTML Format`，RTF 用 `Rich Text Format`，都是各程序约定的注册格式。
//! - 图片：有 `PNG` 注册格式（浏览器、Office、截图工具都会放）就直接用，否则把 `CF_DIB` 位图转成 PNG。
//!   写入时同时放 `PNG` 和 `CF_DIB`：认 PNG 的程序拿到透明度，其余的程序拿位图。
//!
//! 剪贴板打开期间别的程序都用不了它，所以打开时只拷贝原始数据，关上之后再转换。

use std::sync::OnceLock;
use std::thread::sleep;
use std::time::Duration;

use ferry_core::clip_format::{
    dib_to_png, html_from_windows, html_to_windows, png_size, png_to_dib,
};
use ferry_platform::{Clipboard, Error, Result};
use ferry_types::{ClipboardContent, ClipboardFormat};
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber,
    IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{
    GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
};
use windows::core::w;

/// `CF_UNICODETEXT`、`CF_DIB`。为两个常量引入整个 OLE 模块不划算，直接写数值。
const CF_UNICODETEXT: u32 = 13;
const CF_DIB: u32 = 8;

/// 本机能读写的格式。
const FORMATS: &[ClipboardFormat] = &[
    ClipboardFormat::TEXT,
    ClipboardFormat::HTML,
    ClipboardFormat::RTF,
    ClipboardFormat::PNG,
];

/// 剪贴板被别的程序占用时，重试打开的次数和间隔。
const OPEN_ATTEMPTS: u32 = 10;
const OPEN_RETRY_DELAY: Duration = Duration::from_millis(5);

/// 注册格式的编号，每次启动由系统分配。
struct Registered {
    html: u32,
    rtf: u32,
    png: u32,
}

fn registered() -> &'static Registered {
    static FORMATS: OnceLock<Registered> = OnceLock::new();
    // SAFETY: 参数是以 0 结尾的常量字符串。注册失败时为 0，读写时自然找不到这个格式。
    FORMATS.get_or_init(|| unsafe {
        Registered {
            html: RegisterClipboardFormatW(w!("HTML Format")),
            rtf: RegisterClipboardFormatW(w!("Rich Text Format")),
            png: RegisterClipboardFormatW(w!("PNG")),
        }
    })
}

/// 系统剪贴板。
#[derive(Clone, Copy, Debug, Default)]
pub struct WinClipboard;

impl WinClipboard {
    pub fn new() -> Self {
        Self
    }
}

/// 剪贴板打开期间拷出来的原始数据。
#[derive(Default)]
struct Raw {
    text: Option<Vec<u8>>,
    html: Option<Vec<u8>>,
    rtf: Option<Vec<u8>>,
    png: Option<Vec<u8>>,
    dib: Option<Vec<u8>>,
}

impl Clipboard for WinClipboard {
    fn formats(&self) -> &'static [ClipboardFormat] {
        FORMATS
    }

    fn change_count(&mut self) -> u64 {
        // SAFETY: 没有参数，只读一个计数。
        u64::from(unsafe { GetClipboardSequenceNumber() })
    }

    fn read(&mut self) -> Result<ClipboardContent> {
        let formats = registered();
        let raw = {
            let _open = Opened::open()?;
            let png = copy_out(formats.png);
            Raw {
                text: copy_out(CF_UNICODETEXT),
                html: copy_out(formats.html),
                rtf: copy_out(formats.rtf),
                // 有 PNG 就不必再拷一份位图。
                dib: if png.is_none() {
                    copy_out(CF_DIB)
                } else {
                    None
                },
                png,
            }
        };

        let mut content = ClipboardContent::new();
        if let Some(bytes) = raw.text {
            let units: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|p| u16::from_le_bytes([p[0], p[1]]))
                .collect();
            content.insert(
                ClipboardFormat::TEXT,
                text_from_windows(&units).into_bytes(),
            );
        }
        if let Some(html) = raw.html.as_deref().and_then(html_from_windows) {
            content.insert(ClipboardFormat::HTML, html.into_bytes());
        }
        if let Some(rtf) = raw.rtf {
            content.insert(ClipboardFormat::RTF, until_zero(rtf));
        }
        let png = match raw.png.map(until_png_end) {
            Some(png) if png_size(&png).is_some() => Some(png),
            _ => raw.dib.as_deref().and_then(dib_to_png),
        };
        if let Some(png) = png {
            content.insert(ClipboardFormat::PNG, png);
        }
        Ok(content)
    }

    fn write(&mut self, content: &ClipboardContent) -> Result<u64> {
        let formats = registered();
        // 先把要放的内容都准备好，剪贴板只打开一小会儿。
        let mut blocks: Vec<(u32, HGLOBAL)> = Vec::new();
        let prepared = (|| {
            if let Some(text) = content.as_text() {
                blocks.push((CF_UNICODETEXT, global_text(text)?));
            }
            if let Some(html) = content.get(ClipboardFormat::HTML) {
                let data = html_to_windows(&String::from_utf8_lossy(html));
                blocks.push((formats.html, global_bytes(&data, true)?));
            }
            if let Some(rtf) = content.get(ClipboardFormat::RTF) {
                blocks.push((formats.rtf, global_bytes(rtf, true)?));
            }
            if let Some(png) = content.get(ClipboardFormat::PNG) {
                blocks.push((formats.png, global_bytes(png, false)?));
                if let Some(dib) = png_to_dib(png) {
                    blocks.push((CF_DIB, global_bytes(&dib, false)?));
                }
            }
            Ok(())
        })();
        if let Err(e) = prepared {
            free_all(blocks);
            return Err(e);
        }

        let open = match Opened::open() {
            Ok(open) => open,
            Err(e) => {
                free_all(blocks);
                return Err(e);
            }
        };
        // SAFETY: 剪贴板已打开。
        if let Err(e) = unsafe { EmptyClipboard() } {
            free_all(blocks);
            return Err(os("EmptyClipboard", e));
        }
        let mut blocks = blocks.into_iter();
        while let Some((format, memory)) = blocks.next() {
            // SAFETY: 成功后内存块归剪贴板所有，不能再释放；失败时仍归本进程，由 free 释放。
            if let Err(e) = unsafe { SetClipboardData(format, Some(HANDLE(memory.0))) } {
                free(Some(memory));
                free_all(blocks.collect());
                return Err(os("SetClipboardData", e));
            }
        }
        drop(open);
        Ok(self.change_count())
    }
}

/// 把协议中的文本（`\n` 换行）转成 Windows 剪贴板文本：`\r\n` 换行、UTF-16、以 0 结尾。
pub fn text_to_windows(text: &str) -> Vec<u16> {
    let mut units = Vec::with_capacity(text.len() + 1);
    let mut previous = '\0';
    for c in text.chars() {
        if c == '\n' && previous != '\r' {
            units.push(u16::from(b'\r'));
        }
        let mut buf = [0; 2];
        units.extend_from_slice(c.encode_utf16(&mut buf));
        previous = c;
    }
    units.push(0);
    units
}

/// 把 Windows 剪贴板文本转成协议中的文本：截到第一个 0，`\r\n` 换成 `\n`。
pub fn text_from_windows(units: &[u16]) -> String {
    let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
    String::from_utf16_lossy(&units[..end]).replace("\r\n", "\n")
}

/// 打开期间独占系统剪贴板，离开作用域时关闭。
struct Opened;

impl Opened {
    fn open() -> Result<Self> {
        let mut attempt = 1;
        loop {
            // SAFETY: 不指定所属窗口，剪贴板归当前线程打开。
            match unsafe { OpenClipboard(None) } {
                Ok(()) => return Ok(Self),
                Err(_) if attempt < OPEN_ATTEMPTS => {
                    attempt += 1;
                    sleep(OPEN_RETRY_DELAY);
                }
                Err(e) => return Err(os("OpenClipboard", e)),
            }
        }
    }
}

impl Drop for Opened {
    fn drop(&mut self) {
        // SAFETY: 只有成功打开后才会构造 Opened。
        let _ = unsafe { CloseClipboard() };
    }
}

/// 把文本复制进一块新分配的可移动全局内存。
fn global_text(text: &str) -> Result<HGLOBAL> {
    let units = text_to_windows(text);
    let bytes = units.len() * 2;
    // SAFETY: 分配后立即锁定，复制的长度不超过分配的大小。
    unsafe {
        let memory = GlobalAlloc(GMEM_MOVEABLE, bytes).map_err(|e| os("GlobalAlloc", e))?;
        let ptr = GlobalLock(memory).cast::<u16>();
        if ptr.is_null() {
            let error = windows::core::Error::from_thread();
            free(Some(memory));
            return Err(os("GlobalLock", error));
        }
        std::ptr::copy_nonoverlapping(units.as_ptr(), ptr, units.len());
        let _ = GlobalUnlock(memory);
        Ok(memory)
    }
}

/// 把字节复制进一块新分配的可移动全局内存。`nul`：末尾补一个 0，文本类的格式都以 0 结尾。
fn global_bytes(data: &[u8], nul: bool) -> Result<HGLOBAL> {
    let len = data.len() + usize::from(nul);
    // SAFETY: 分配后立即锁定，复制的长度不超过分配的大小。
    unsafe {
        let memory = GlobalAlloc(GMEM_MOVEABLE, len).map_err(|e| os("GlobalAlloc", e))?;
        let ptr = GlobalLock(memory).cast::<u8>();
        if ptr.is_null() {
            let error = windows::core::Error::from_thread();
            free(Some(memory));
            return Err(os("GlobalLock", error));
        }
        std::ptr::copy_nonoverlapping(data.as_ptr(), ptr, data.len());
        if nul {
            *ptr.add(data.len()) = 0;
        }
        let _ = GlobalUnlock(memory);
        Ok(memory)
    }
}

/// 剪贴板打开期间，拷出一种格式的原始数据。没有这种格式或读不出来时为 `None`。
fn copy_out(format: u32) -> Option<Vec<u8>> {
    if format == 0 {
        return None;
    }
    // SAFETY: 调用方已打开剪贴板；返回的句柄归剪贴板所有，只在关闭前读取，
    // 锁定期间指针有效，长度取自 GlobalSize。
    unsafe {
        IsClipboardFormatAvailable(format).ok()?;
        let handle = GetClipboardData(format).ok()?;
        let memory = HGLOBAL(handle.0);
        let ptr = GlobalLock(memory).cast::<u8>();
        if ptr.is_null() {
            return None;
        }
        let bytes = std::slice::from_raw_parts(ptr, GlobalSize(memory)).to_vec();
        let _ = GlobalUnlock(memory);
        Some(bytes)
    }
}

/// 截到第一个 0。系统分配的内存块可能比写入的数据大，后面的字节没有意义。
fn until_zero(mut data: Vec<u8>) -> Vec<u8> {
    if let Some(end) = data.iter().position(|&b| b == 0) {
        data.truncate(end);
    }
    data
}

/// 截到 PNG 的结束块（`IEND` 加 4 字节校验）为止，原因同上。
fn until_png_end(mut png: Vec<u8>) -> Vec<u8> {
    if let Some(at) = png.windows(4).rposition(|w| w == b"IEND") {
        png.truncate((at + 8).min(png.len()));
    }
    png
}

fn free_all(blocks: Vec<(u32, HGLOBAL)>) {
    for (_, memory) in blocks {
        free(Some(memory));
    }
}

fn free(memory: Option<HGLOBAL>) {
    if let Some(memory) = memory {
        // SAFETY: 内存块由本进程分配，且尚未交给剪贴板。
        let _ = unsafe { GlobalFree(Some(memory)) };
    }
}

fn os(call: &'static str, error: windows::core::Error) -> Error {
    Error::Os {
        call,
        detail: error.message(),
    }
}
