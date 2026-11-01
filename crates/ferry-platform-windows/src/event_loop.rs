//! 主线程事件循环：`EventLoop` 的 Windows 实现。
//!
//! 主线程运行消息循环。其他线程的任务投递给一个只收消息的隐藏窗口：
//! 托盘菜单弹出时的模态循环同样会分发窗口消息，任务不会丢。
//!
//! 后台是控制台程序，从资源管理器双击或登录时启动会带出一个黑色的控制台窗口；
//! 创建事件循环时，如果这个控制台只属于本进程，就把它关掉。从终端运行时不受影响。
//! 关掉之后原来的标准输入输出句柄就失效了，一并清空：启动子进程时默认要复制这几个句柄，
//! 复制失效的句柄会让启动失败（设置界面打不开就是这个原因）。

use ferry_platform::{Dispatcher, Error, EventLoop, MainTask, Result};
use windows::Win32::Foundation::{HANDLE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Console::{
    FreeConsole, GetConsoleProcessList, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
    SetStdHandle,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    ASFW_ANY, AllowSetForegroundWindow, CreateWindowExW, DefWindowProcW, DispatchMessageW,
    GetMessageW, HWND_MESSAGE, MSG, PostMessageW, RegisterClassW, TranslateMessage,
    WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WNDCLASSW,
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
        detach_own_console();
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

/// 控制台只连着本进程（不是从终端启动的）时关掉它。
fn detach_own_console() {
    let mut processes = [0u32; 2];
    // SAFETY: 缓冲区有效；返回值是连在这个控制台上的进程数。
    let attached = unsafe { GetConsoleProcessList(&mut processes) };
    if attached == 1 {
        // SAFETY: 只是与控制台断开，之后写到标准输出的内容被丢弃（日志写到文件）；
        // 失效的句柄换成空句柄，子进程就不会继承到坏句柄。
        unsafe {
            let _ = FreeConsole();
            for which in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
                let _ = SetStdHandle(which, HANDLE::default());
            }
        }
    }
}

/// 允许接下来启动的程序把自己的窗口放到最前面。
///
/// 从后台进程启动的窗口默认抢不到前台，只会在任务栏上闪一下。用户刚点过托盘时本进程有前台权限，
/// 在启动设置界面之前把这个权限让出去。
pub fn allow_foreground() {
    // SAFETY: 简单的系统调用；没有权限时失败，不影响什么。
    let _ = unsafe { AllowSetForegroundWindow(ASFW_ANY) };
}

fn os(call: &'static str, e: windows::core::Error) -> Error {
    Error::Os {
        call,
        detail: e.message(),
    }
}
