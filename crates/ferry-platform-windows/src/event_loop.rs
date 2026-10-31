//! 主线程事件循环：`EventLoop` 的 Windows 实现。
//!
//! 主线程运行消息循环。其他线程的任务投递给一个只收消息的隐藏窗口：
//! 托盘菜单弹出时的模态循环同样会分发窗口消息，任务不会丢。

use ferry_platform::{Dispatcher, Error, EventLoop, MainTask, Result};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, HWND_MESSAGE, MSG,
    PostMessageW, RegisterClassW, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP,
    WNDCLASSW,
};
use windows::core::w;

/// 投递任务用的消息。`lparam` 是 `Box<MainTask>` 的裸指针。
const WM_TASK: u32 = WM_APP + 1;

pub struct WinEventLoop {
    hwnd: HWND,
}

impl WinEventLoop {
    /// 只能在主线程上调用：窗口属于创建它的线程，任务也在这个线程上执行。
    pub fn new() -> Result<Self> {
        let class = w!("ScreenFerryMainThread");
        // SAFETY: 参数都是有效的常量或刚取得的句柄；窗口过程只处理本模块投递的消息。
        let hwnd = unsafe {
            let instance = GetModuleHandleW(None).map_err(|e| os("GetModuleHandleW", e))?;
            let wc = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: instance.into(),
                lpszClassName: class,
                ..Default::default()
            };
            RegisterClassW(&wc);
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class,
                w!(""),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                Some(instance.into()),
                None,
            )
            .map_err(|e| os("CreateWindowExW", e))?
        };
        Ok(Self { hwnd })
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_TASK {
        // SAFETY: 这个消息只由 dispatcher 投递，lparam 是 Box::into_raw 得到的指针，只取回一次。
        let task = unsafe { Box::from_raw(lparam.0 as *mut MainTask) };
        task();
        return LRESULT(0);
    }
    // SAFETY: 其余消息交给系统默认处理。
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

impl EventLoop for WinEventLoop {
    fn dispatcher(&self) -> Dispatcher {
        // 窗口句柄本身不是 Send，以整数形式带到其他线程；PostMessageW 可以在任何线程调用。
        let hwnd = self.hwnd.0 as isize;
        Dispatcher::new(move |task| {
            let raw = Box::into_raw(Box::new(task));
            // SAFETY: 窗口在进程结束前一直存在；投递失败时收回指针，避免泄漏。
            let posted = unsafe {
                PostMessageW(
                    Some(HWND(hwnd as *mut _)),
                    WM_TASK,
                    WPARAM(0),
                    LPARAM(raw as isize),
                )
            };
            if posted.is_err() {
                // SAFETY: 投递失败，指针仍归这里所有。
                drop(unsafe { Box::from_raw(raw) });
            }
        })
    }

    /// Windows 上再次运行程序会启动新进程，它经本机通信请这一份打开设置界面，不走这里。
    fn on_reopen(&mut self, _handler: Box<dyn Fn()>) {}

    fn run(self) -> ! {
        let mut msg = MSG::default();
        // SAFETY: 标准的消息循环，msg 指向有效的缓冲区。
        unsafe {
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        std::process::exit(0)
    }
}

fn os(call: &'static str, e: windows::core::Error) -> Error {
    Error::Os {
        call,
        detail: e.message(),
    }
}
