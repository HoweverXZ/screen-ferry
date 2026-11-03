//! 把本机光标藏起来：光标停在一个 1×1 的窗口上，这个窗口的光标是全透明的，于是看不见。
//! 本机用户一动鼠标，光标离开这一个像素就会重新出现，不需要额外处理。
//!
//! 藏着的时候每隔一会儿看一眼：光标还停在那里、却不在这个窗口上（别的置顶窗口后来盖了上来），
//! 就把窗口重新放到最上面、再把光标放回去，让它重新用透明的光标。
//!
//! 收起时必须主动把光标换回底下那个窗口的样子：窗口收起来、光标从透明的窗口上挪开，
//! 系统都不一定重新问一遍“这里该用哪个光标”，光标就会一直保持全透明，用户找不到指针。
//!
//! 窗口要有消息循环才能设置光标形状，所以它跑在自己的线程上。藏起和收起都同步等它做完：
//! 调用方紧接着注入的移动必须排在后面，否则光标刚移到入口，又被挪回屏幕中央。

use std::cell::Cell;
use std::sync::mpsc;
use std::thread;

use ferry_platform::{Error, Result};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateCursor, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetCursorPos,
    GetMessageW, GetSystemMetrics, HCURSOR, HTCLIENT, HWND_TOPMOST, KillTimer, MSG, PostMessageW,
    PostQuitMessage, RegisterClassW, SM_CXCURSOR, SM_CXSCREEN, SM_CYCURSOR, SM_CYSCREEN,
    SMTO_ABORTIFHUNG, SW_HIDE, SWP_NOACTIVATE, SWP_SHOWWINDOW, SendMessageTimeoutW, SetCursor,
    SetCursorPos, SetTimer, SetWindowPos, ShowWindow, TranslateMessage, WM_APP, WM_DESTROY,
    WM_MOUSEMOVE, WM_SETCURSOR, WM_TIMER, WNDCLASSW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_POPUP, WindowFromPoint,
};
use windows::core::w;

/// 把窗口放到屏幕中央、光标移上去。
const WM_HIDE_CURSOR: u32 = WM_APP + 10;
/// 收起窗口，并且把光标换回底下那个窗口的样子。
const WM_SHOW_CURSOR: u32 = WM_APP + 11;
/// 销毁窗口，结束线程。
const WM_STOP: u32 = WM_APP + 12;
/// 等隐藏窗口处理完一条消息最多等这么久（毫秒）。正常情况下不到一毫秒。
const SEND_TIMEOUT_MS: u32 = 200;
/// 藏着的时候多久看一眼（毫秒）。
const CHECK_MS: u32 = 250;
/// 等光标底下的窗口换回自己的光标最多等这么久（毫秒）。别等太久，藏起和收起都在等这条消息。
const RESTORE_TIMEOUT_MS: u32 = 50;
const CHECK_TIMER: usize = 1;

thread_local! {
    /// 藏着的时候光标停在哪里；没藏时为 `None`。只在窗口所在的线程上读写。
    static PARKED: Cell<Option<(i32, i32)>> = const { Cell::new(None) };
    /// 全透明的光标形状。藏的时候显式换上，不指望系统自己来问。
    static BLANK: Cell<isize> = const { Cell::new(0) };
}

#[derive(Debug)]
pub(crate) struct CursorHider {
    /// 窗口句柄本身不是 Send，以整数保存；给窗口发消息可以在任何线程进行。
    hwnd: isize,
}

impl CursorHider {
    pub(crate) fn start() -> Result<Self> {
        let (tx, rx) = mpsc::channel();
        thread::Builder::new()
            .name("ferry-cursor".into())
            .spawn(move || {
                let hwnd = match create_window() {
                    Ok(hwnd) => hwnd,
                    Err(e) => {
                        let _ = tx.send(Err(e));
                        return;
                    }
                };
                let _ = tx.send(Ok(hwnd.0 as isize));
                let mut msg = MSG::default();
                // SAFETY: 标准的消息循环，msg 指向有效的缓冲区。
                unsafe {
                    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
            })
            .map_err(|e| Error::Os {
                call: "thread::spawn",
                detail: e.to_string(),
            })?;
        let hwnd = rx.recv().map_err(|_| Error::Os {
            call: "CreateWindowExW",
            detail: "隐藏光标的线程意外退出".into(),
        })??;
        Ok(Self { hwnd })
    }

    /// 藏起或收起，返回时已经做完。
    pub(crate) fn set_hidden(&self, hidden: bool) -> Result<()> {
        let msg = if hidden {
            WM_HIDE_CURSOR
        } else {
            WM_SHOW_CURSOR
        };
        // SAFETY: 窗口在本对象存在期间一直存在；不需要结果，结果指针为空。
        let done = unsafe {
            SendMessageTimeoutW(
                HWND(self.hwnd as *mut _),
                msg,
                WPARAM(0),
                LPARAM(0),
                SMTO_ABORTIFHUNG,
                SEND_TIMEOUT_MS,
                None,
            )
        };
        if done.0 == 0 {
            return Err(Error::Os {
                call: "SendMessageTimeoutW",
                detail: "隐藏光标的窗口没有及时响应".into(),
            });
        }
        Ok(())
    }

    fn post(&self, msg: u32) -> Result<()> {
        // SAFETY: 窗口在本对象存在期间一直存在。
        unsafe { PostMessageW(Some(HWND(self.hwnd as *mut _)), msg, WPARAM(0), LPARAM(0)) }
            .map_err(|e| os("PostMessageW", e))
    }
}

impl Drop for CursorHider {
    fn drop(&mut self) {
        // 先让光标回来再销毁窗口：窗口没了，那个全透明的光标形状还留在屏幕上。
        let _ = self.post(WM_SHOW_CURSOR);
        let _ = self.post(WM_STOP);
    }
}

/// 在当前线程创建隐藏用的窗口：1×1、置顶、不在任务栏、不抢焦点，类光标是全透明的。
fn create_window() -> Result<HWND> {
    // SAFETY: 参数都是有效的常量、刚取得的句柄或在调用期间有效的缓冲区。
    unsafe {
        let instance = GetModuleHandleW(None).map_err(|e| os("GetModuleHandleW", e))?;
        let (width, height) = (
            GetSystemMetrics(SM_CXCURSOR).max(1),
            GetSystemMetrics(SM_CYCURSOR).max(1),
        );
        // 单色位图每行按 16 位对齐。AND 全 1、XOR 全 0：每个像素都透明。
        let stride = (width as usize).div_ceil(16) * 2;
        let and_mask = vec![0xFF_u8; stride * height as usize];
        let xor_mask = vec![0_u8; stride * height as usize];
        let blank = CreateCursor(
            Some(instance.into()),
            0,
            0,
            width,
            height,
            and_mask.as_ptr().cast(),
            xor_mask.as_ptr().cast(),
        )
        .map_err(|e| os("CreateCursor", e))?;
        BLANK.with(|b| b.set(blank.0 as isize));

        let class = w!("ScreenFerryCursorHider");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance.into(),
            hCursor: blank,
            lpszClassName: class,
            ..Default::default()
        };
        RegisterClassW(&wc);
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class,
            w!(""),
            WS_POPUP,
            0,
            0,
            1,
            1,
            None,
            None,
            Some(instance.into()),
            None,
        )
        .map_err(|e| os("CreateWindowExW", e))
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: 都是对本线程自己的窗口和光标的操作。
    unsafe {
        match msg {
            WM_HIDE_CURSOR => {
                let (x, y) = (
                    GetSystemMetrics(SM_CXSCREEN) / 2,
                    GetSystemMetrics(SM_CYSCREEN) / 2,
                );
                park(hwnd, x, y);
                PARKED.with(|p| p.set(Some((x, y))));
                SetTimer(Some(hwnd), CHECK_TIMER, CHECK_MS, None);
                LRESULT(0)
            }
            WM_SHOW_CURSOR => {
                PARKED.with(|p| p.set(None));
                let _ = KillTimer(Some(hwnd), CHECK_TIMER);
                let _ = ShowWindow(hwnd, SW_HIDE);
                // 窗口收起来之后系统不会自己把光标换回底下那个窗口的样子。
                restore_cursor();
                LRESULT(0)
            }
            WM_TIMER if wparam.0 == CHECK_TIMER => {
                let Some((x, y)) = PARKED.with(Cell::get) else {
                    let _ = KillTimer(Some(hwnd), CHECK_TIMER);
                    return LRESULT(0);
                };
                let mut now = POINT::default();
                if GetCursorPos(&mut now).is_ok() && (now.x, now.y) != (x, y) {
                    // 光标被挪走了（多半是本机的人自己动了鼠标）：它本来就该出现，不再看着。
                    // 窗口也要收起来，否则屏幕中央一直留着一个看不见的置顶窗口，
                    // 远端的光标经过这一像素时会突然消失。
                    PARKED.with(|p| p.set(None));
                    let _ = KillTimer(Some(hwnd), CHECK_TIMER);
                    let _ = ShowWindow(hwnd, SW_HIDE);
                    restore_cursor();
                } else if WindowFromPoint(POINT { x, y }) != hwnd {
                    // 别的置顶窗口盖了上来，光标用的是它的样子。
                    park(hwnd, x, y);
                }
                LRESULT(0)
            }
            WM_STOP => {
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

/// 把窗口放到最上面的 (x, y)，光标移上去。移动光标会让窗口收到 WM_SETCURSOR，换成透明的光标；
/// 光标本来就在那一点时不会收到，所以显式再换一次。
///
/// # Safety
///
/// `hwnd` 必须是本线程创建的隐藏窗口。
unsafe fn park(hwnd: HWND, x: i32, y: i32) {
    // SAFETY: 调用方保证窗口有效且属于本线程。
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            x,
            y,
            1,
            1,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        let _ = SetCursorPos(x, y);
        let blank = BLANK.with(Cell::get);
        if blank != 0 {
            // 光标本来就在这一点时 SetCursorPos 不会产生 WM_SETCURSOR，显式换上透明的那个。
            let _ = SetCursor(Some(HCURSOR(blank as _)));
        }
    }
}

/// 让光标换回它底下那个窗口自己的样子。
///
/// 光标停过一个全透明的光标形状之后，收窗口、挪走光标都不一定让它变回来：系统只在光标
/// 真的换了一个窗口时才会重新问一次。这里主动给光标底下的窗口发一条 `WM_SETCURSOR`，
/// 让它把光标设回去；对方不理这条消息时，再把光标挪开一个像素、挪回来，让系统自己重问一遍。
///
/// # Safety
///
/// 只读本线程所在桌面上的光标位置和窗口，消息发给那个窗口自己的线程。
unsafe fn restore_cursor() {
    // SAFETY: 调用方保证在本线程、处理自己的窗口消息。
    unsafe {
        let mut at = POINT::default();
        if GetCursorPos(&mut at).is_err() {
            return;
        }
        let under = WindowFromPoint(at);
        // WM_SETCURSOR 的低 16 位是命中的区域，高 16 位是鼠标消息。
        let lparam = (HTCLIENT as i32 & 0xFFFF) | ((WM_MOUSEMOVE as i32) << 16);
        let done = if under.0.is_null() {
            LRESULT(0)
        } else {
            SendMessageTimeoutW(
                under,
                WM_SETCURSOR,
                WPARAM(under.0 as _),
                LPARAM(lparam as _),
                SMTO_ABORTIFHUNG,
                RESTORE_TIMEOUT_MS,
                None,
            )
        };
        if done.0 == 0 {
            // 那个窗口没理会（卡住、或者不是本程序的窗口）：让系统自己再问一遍。
            let _ = SetCursorPos(at.x + 1, at.y);
            let _ = SetCursorPos(at.x, at.y);
        }
    }
}

fn os(call: &'static str, e: windows::core::Error) -> Error {
    Error::Os {
        call,
        detail: e.message(),
    }
}
