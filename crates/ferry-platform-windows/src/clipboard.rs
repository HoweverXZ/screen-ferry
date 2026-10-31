//! 剪贴板：`Clipboard` 的 Windows 实现，基于 Win32 剪贴板 API。
//!
//! 文本以 `CF_UNICODETEXT`（UTF-16）读写，系统会自动为旧程序生成其他文本格式。
//! 协议中的文本换行是 `\n`，Windows 习惯 `\r\n`，进出时各转换一次。

use std::thread::sleep;
use std::time::Duration;

use ferry_platform::{Clipboard, Error, Result};
use ferry_types::{ClipboardContent, ClipboardFormat};
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber,
    IsClipboardFormatAvailable, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{
    GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
};

/// `CF_UNICODETEXT`。为一个常量引入整个 OLE 模块不划算，直接写数值。
const CF_UNICODETEXT: u32 = 13;

/// 剪贴板被别的程序占用时，重试打开的次数和间隔。
const OPEN_ATTEMPTS: u32 = 10;
const OPEN_RETRY_DELAY: Duration = Duration::from_millis(5);

/// 系统剪贴板。
#[derive(Clone, Copy, Debug, Default)]
pub struct WinClipboard;

impl WinClipboard {
    pub fn new() -> Self {
        Self
    }
}

impl Clipboard for WinClipboard {
    fn formats(&self) -> &'static [ClipboardFormat] {
        &[ClipboardFormat::TEXT]
    }

    fn change_count(&mut self) -> u64 {
        // SAFETY: 没有参数，只读一个计数。
        u64::from(unsafe { GetClipboardSequenceNumber() })
    }

    fn read(&mut self) -> Result<ClipboardContent> {
        let _open = Opened::open()?;
        // SAFETY: 剪贴板已打开。
        if unsafe { IsClipboardFormatAvailable(CF_UNICODETEXT) }.is_err() {
            return Ok(ClipboardContent::new());
        }
        // SAFETY: 剪贴板已打开；返回的句柄归剪贴板所有，只在关闭前读取。
        let handle =
            unsafe { GetClipboardData(CF_UNICODETEXT) }.map_err(|e| os("GetClipboardData", e))?;
        let memory = HGLOBAL(handle.0);
        // SAFETY: memory 是剪贴板中的全局内存块，锁定期间指针有效，长度取自 GlobalSize。
        let text = unsafe {
            let ptr = GlobalLock(memory).cast::<u16>();
            if ptr.is_null() {
                return Err(os("GlobalLock", windows::core::Error::from_thread()));
            }
            let units = std::slice::from_raw_parts(ptr, GlobalSize(memory) / 2);
            let text = text_from_windows(units);
            let _ = GlobalUnlock(memory);
            text
        };
        Ok(ClipboardContent::text(text))
    }

    fn write(&mut self, content: &ClipboardContent) -> Result<u64> {
        let memory = match content.as_text() {
            Some(text) => Some(global_text(text)?),
            None => None,
        };
        let open = match Opened::open() {
            Ok(open) => open,
            Err(e) => {
                free(memory);
                return Err(e);
            }
        };
        // SAFETY: 剪贴板已打开。
        if let Err(e) = unsafe { EmptyClipboard() } {
            free(memory);
            return Err(os("EmptyClipboard", e));
        }
        if let Some(memory) = memory {
            // SAFETY: 成功后内存块归剪贴板所有，不能再释放；失败时仍归本进程，由 free 释放。
            if let Err(e) = unsafe { SetClipboardData(CF_UNICODETEXT, Some(HANDLE(memory.0))) } {
                free(Some(memory));
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
