//! 显示器信息：`Displays` 的 Windows 实现。
//!
//! 坐标使用 Windows 虚拟桌面坐标：以主显示器左上角为原点，单位为物理像素。

use std::sync::Once;

use ferry_platform::{Displays, Error, Result};
use ferry_types::{Display, DisplayId, Rect};
use windows::Win32::Foundation::{LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForMonitor, MDT_EFFECTIVE_DPI,
    SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::MONITORINFOF_PRIMARY;
use windows::core::BOOL;

/// Windows 的基准 DPI，对应 100% 缩放。
const BASE_DPI: u32 = 96;

/// 本机显示器列表。
#[derive(Clone, Copy, Debug)]
pub struct WinDisplays(());

impl WinDisplays {
    pub fn new() -> Self {
        ensure_dpi_aware();
        Self(())
    }
}

impl Default for WinDisplays {
    fn default() -> Self {
        Self::new()
    }
}

impl Displays for WinDisplays {
    fn displays(&self) -> Result<Vec<Display>> {
        let mut found: Vec<Display> = Vec::new();
        // SAFETY: 回调只在本次调用期间运行，`found` 在此期间有效且不被其他代码访问。
        let ok = unsafe {
            EnumDisplayMonitors(
                None,
                None,
                Some(on_monitor),
                LPARAM(&raw mut found as isize),
            )
        };
        if !ok.as_bool() {
            return Err(last_error("EnumDisplayMonitors"));
        }
        Ok(found)
    }
}

unsafe extern "system" fn on_monitor(
    monitor: HMONITOR,
    _hdc: HDC,
    _clip: *mut RECT,
    data: LPARAM,
) -> BOOL {
    // SAFETY: `data` 是 `displays` 中传入的 `&mut Vec<Display>`。
    let found = unsafe { &mut *(data.0 as *mut Vec<Display>) };

    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: `info` 是正确初始化了 `cbSize` 的本地变量。
    if unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        let (mut dpi_x, mut dpi_y) = (BASE_DPI, BASE_DPI);
        // SAFETY: 两个指针指向本地变量。
        let scale_percent =
            match unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) } {
                Ok(()) => u16::try_from(dpi_x * 100 / BASE_DPI).unwrap_or(100),
                Err(_) => 100,
            };
        let r = info.rcMonitor;
        found.push(Display {
            id: DisplayId(found.len() as u32),
            bounds: Rect::new(
                r.left,
                r.top,
                r.right.abs_diff(r.left),
                r.bottom.abs_diff(r.top),
            ),
            scale_percent,
            primary: info.dwFlags & MONITORINFOF_PRIMARY != 0,
        });
    }
    BOOL(1)
}

/// 让进程按物理像素工作。整个进程只需设置一次，重复调用无害。
pub(crate) fn ensure_dpi_aware() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        // SAFETY: 只修改本进程的 DPI 感知模式。已经设置过（例如由清单设置）时会失败，可以忽略。
        let _ =
            unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    });
}

pub(crate) fn last_error(call: &'static str) -> Error {
    Error::Os {
        call,
        detail: std::io::Error::last_os_error().to_string(),
    }
}
