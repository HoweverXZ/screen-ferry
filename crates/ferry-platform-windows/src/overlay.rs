//! 屏幕边缘浮层：`Overlay` 的 Windows 实现。
//!
//! 一个置顶、不抢焦点、不在任务栏里的分层窗口（layered window），跑在自己的界面线程上，
//! 用系统的 OLE 拖放接口接收文件。它有两种样子：
//!
//! - 显示胶囊：用 Direct2D 和 DirectWrite 照着 [`Scene`] 画进一张 32 位位图，按逐像素透明度贴到屏幕上；
//! - 留意贴边区域：同一个窗口变成贴着边的一条、看不见的接收条。Windows 没法提前知道别的程序在拖什么，
//!   拖进这里的是文件，才知道用户在往对方那边拖文件。
//!
//! 两种样子是同一个窗口，接收条变成胶囊时拖放不会被打断。整个窗口铺了一层几乎透明的底色，
//! 所以哪里都能接住拖放，一直延伸到屏幕边缘。其他线程只改要显示的内容，再通知界面线程。

mod drop;
mod paint;

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, mpsc};
use std::thread;

use ferry_platform::{Error, Overlay, OverlayEvent, OverlaySink, Result};
use ferry_types::{Frame, Scene};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Ole::{IDropTarget, OleInitialize, RegisterDragDrop, RevokeDragDrop};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, IDC_ARROW,
    LoadCursorW, MA_NOACTIVATE, MSG, PostMessageW, PostQuitMessage, RegisterClassW, SW_HIDE,
    SW_SHOWNORMAL, ShowWindow, TranslateMessage, WM_APP, WM_DESTROY, WM_LBUTTONDOWN,
    WM_MOUSEACTIVATE, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_POPUP,
};
use windows::core::{HSTRING, w};

use drop::DropTarget;
use paint::Painter;

/// 要显示的内容变了。
const WM_UPDATE: u32 = WM_APP + 20;
/// 有文件要打开或在资源管理器里显示。
const WM_SHELL: u32 = WM_APP + 21;
/// 销毁窗口，结束线程。
const WM_STOP: u32 = WM_APP + 22;

/// 界面线程与调用方共享的东西。
pub(crate) struct Shared {
    /// 浮层窗口的句柄（以整数保存）；还没建好时为 0。
    hwnd: AtomicIsize,
    wanted: Mutex<Wanted>,
    pub(crate) drag: Mutex<DragState>,
    sink: Mutex<Option<OverlaySink>>,
}

/// 调用方想要的样子。
#[derive(Default)]
struct Wanted {
    /// 显示胶囊：窗口位置、每点多少像素、内容。
    scene: Option<(Frame, f32, Scene)>,
    /// 没有胶囊时留意的贴边区域。
    strip: Option<Frame>,
    /// 要打开（`false`）或在资源管理器里显示（`true`）的文件。
    shell: Vec<(PathBuf, bool)>,
}

/// 最近一次拖进来的文件。
#[derive(Default)]
pub(crate) struct DragState {
    /// 每有一次拖着文件进来就加一。
    pub(crate) count: i64,
    pub(crate) files: Vec<PathBuf>,
    /// 现在拖着的东西是不是文件（决定鼠标显示为“复制”还是“不可放”）。
    pub(crate) holding_files: bool,
}

impl Shared {
    pub(crate) fn emit(&self, event: OverlayEvent) {
        if let Some(sink) = lock(&self.sink).as_mut() {
            sink(event);
        }
    }

    fn post(&self, msg: u32) {
        let hwnd = self.hwnd.load(Ordering::Acquire);
        if hwnd != 0 {
            // SAFETY: 窗口在界面线程结束前一直存在；投递失败（窗口已关）可以忽略。
            let _ = unsafe { PostMessageW(Some(HWND(hwnd as *mut _)), msg, WPARAM(0), LPARAM(0)) };
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// 屏幕边缘的浮层。
pub struct WinOverlay {
    shared: Arc<Shared>,
}

impl WinOverlay {
    pub fn new() -> Self {
        Self {
            shared: Arc::new(Shared {
                hwnd: AtomicIsize::new(0),
                wanted: Mutex::new(Wanted::default()),
                drag: Mutex::new(DragState::default()),
                sink: Mutex::new(None),
            }),
        }
    }

    fn update(&self, change: impl FnOnce(&mut Wanted)) {
        change(&mut lock(&self.shared.wanted));
        self.shared.post(WM_UPDATE);
    }
}

impl Default for WinOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for WinOverlay {
    fn drop(&mut self) {
        self.shared.post(WM_STOP);
    }
}

impl Overlay for WinOverlay {
    fn start(&mut self, sink: OverlaySink) -> Result<()> {
        *lock(&self.shared.sink) = Some(sink);
        let shared = self.shared.clone();
        let (ready_tx, ready_rx) = mpsc::channel();
        thread::Builder::new()
            .name("ferry-overlay".into())
            .spawn(move || ui_thread(shared, ready_tx))
            .map_err(|e| Error::Os {
                call: "thread::spawn",
                detail: e.to_string(),
            })?;
        ready_rx.recv().map_err(|_| Error::Os {
            call: "CreateWindowExW",
            detail: "浮层线程意外退出".into(),
        })?
    }

    fn show(&mut self, frame: Frame, scale: f32, scene: Scene) {
        self.update(|w| w.scene = Some((frame, scale, scene)));
    }

    fn hide(&mut self) {
        self.update(|w| w.scene = None);
    }

    fn drag_count(&self) -> i64 {
        lock(&self.shared.drag).count
    }

    fn dragged_files(&self) -> Vec<PathBuf> {
        lock(&self.shared.drag).files.clone()
    }

    fn watch_edge(&mut self, strip: Option<Frame>) {
        let same = lock(&self.shared.wanted).strip == strip;
        if !same {
            self.update(|w| w.strip = strip);
        }
    }

    fn open(&self, path: &Path) {
        lock(&self.shared.wanted)
            .shell
            .push((path.to_owned(), false));
        self.shared.post(WM_SHELL);
    }

    fn reveal(&self, path: &Path) {
        lock(&self.shared.wanted)
            .shell
            .push((path.to_owned(), true));
        self.shared.post(WM_SHELL);
    }
}

/// 界面线程独占的状态，窗口过程经它找到画笔和当前内容。
struct Ui {
    shared: Arc<Shared>,
    painter: Painter,
    /// 当前显示的胶囊（用于判断点击落在哪里）。
    current: Option<(f32, Scene)>,
    /// 拖放目标，窗口存在期间保持存活。
    _target: IDropTarget,
}

thread_local! {
    static UI: RefCell<Option<Ui>> = const { RefCell::new(None) };
}

fn ui_thread(shared: Arc<Shared>, ready: mpsc::Sender<Result<()>>) {
    let created = (|| -> Result<HWND> {
        // SAFETY: 本线程从这里开始使用 OLE（拖放要求单线程套间）。
        unsafe { OleInitialize(None) }.map_err(|e| os("OleInitialize", e))?;
        let hwnd = create_window()?;
        let target: IDropTarget = DropTarget::new(shared.clone()).into();
        // SAFETY: 窗口由本线程创建，目标对象在窗口存在期间一直存活。
        unsafe { RegisterDragDrop(hwnd, &target) }.map_err(|e| os("RegisterDragDrop", e))?;
        let painter = Painter::new()?;
        UI.with(|ui| {
            *ui.borrow_mut() = Some(Ui {
                shared: shared.clone(),
                painter,
                current: None,
                _target: target,
            });
        });
        Ok(hwnd)
    })();
    let hwnd = match created {
        Ok(hwnd) => hwnd,
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };
    shared.hwnd.store(hwnd.0 as isize, Ordering::Release);
    let _ = ready.send(Ok(()));

    let mut msg = MSG::default();
    // SAFETY: 标准的消息循环，msg 指向有效的缓冲区。
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    shared.hwnd.store(0, Ordering::Release);
    UI.with(|ui| ui.borrow_mut().take());
}

fn create_window() -> Result<HWND> {
    // SAFETY: 参数都是有效的常量或刚取得的句柄。
    unsafe {
        let instance = GetModuleHandleW(None).map_err(|e| os("GetModuleHandleW", e))?;
        let class = w!("ScreenFerryEdgeOverlay");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: class,
            ..Default::default()
        };
        RegisterClassW(&wc);
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
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
    match msg {
        WM_UPDATE => {
            with_ui(|ui| ui.apply(hwnd));
            LRESULT(0)
        }
        WM_SHELL => {
            with_ui(|ui| ui.run_shell());
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            // 坐标是窗口内的像素，低 16 位为 x、高 16 位为 y（有符号）。
            let x = f32::from((lparam.0 & 0xFFFF) as u16 as i16);
            let y = f32::from(((lparam.0 >> 16) & 0xFFFF) as u16 as i16);
            with_ui(|ui| ui.click(x, y));
            LRESULT(0)
        }
        // 点击时不激活窗口，焦点留在用户原来的窗口上。
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_STOP => {
            // SAFETY: 窗口由本线程创建。
            unsafe {
                let _ = RevokeDragDrop(hwnd);
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            // SAFETY: 结束本线程的消息循环。
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        // SAFETY: 其余消息交给系统的默认处理。
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn with_ui(work: impl FnOnce(&mut Ui)) {
    UI.with(|ui| {
        if let Some(ui) = ui.borrow_mut().as_mut() {
            work(ui);
        }
    });
}

impl Ui {
    /// 按调用方想要的样子摆放并画出窗口。
    fn apply(&mut self, hwnd: HWND) {
        let (scene, strip) = {
            let wanted = lock(&self.shared.wanted);
            (wanted.scene.clone(), wanted.strip)
        };
        let shown = match (&scene, strip) {
            (Some((frame, scale, scene)), _) => {
                self.painter.present(hwnd, *frame, *scale, Some(scene))
            }
            (None, Some(strip)) => self.painter.present(hwnd, strip, 1.0, None),
            (None, None) => {
                // SAFETY: 窗口由本线程创建。
                let _ = unsafe { ShowWindow(hwnd, SW_HIDE) };
                Ok(())
            }
        };
        if let Err(e) = shown {
            // 画不出来只影响显示；收起窗口，免得留下一块看不见却挡着拖放的区域。
            let _ = e;
            // SAFETY: 同上。
            let _ = unsafe { ShowWindow(hwnd, SW_HIDE) };
        }
        self.current = scene.map(|(_, scale, scene)| (scale, scene));
    }

    fn click(&mut self, x: f32, y: f32) {
        let Some((scale, scene)) = &self.current else {
            return;
        };
        let (x, y) = (x / scale.max(0.01), y / scale.max(0.01));
        if let Some(hit) = scene.hits.iter().find(|hit| hit.frame.contains(x, y)) {
            self.shared.emit(OverlayEvent::Clicked(hit.id));
        }
    }

    fn run_shell(&mut self) {
        let requests = std::mem::take(&mut lock(&self.shared.wanted).shell);
        for (path, reveal) in requests {
            let path = path.to_string_lossy().into_owned();
            // SAFETY: 字符串在调用期间有效；打开失败只是没有反应。
            unsafe {
                if reveal {
                    let args = HSTRING::from(format!("/select,\"{path}\""));
                    ShellExecuteW(
                        None,
                        w!("open"),
                        w!("explorer.exe"),
                        &args,
                        None,
                        SW_SHOWNORMAL,
                    );
                } else {
                    ShellExecuteW(
                        None,
                        w!("open"),
                        &HSTRING::from(path),
                        None,
                        None,
                        SW_SHOWNORMAL,
                    );
                }
            }
        }
    }
}

pub(crate) fn os(call: &'static str, e: windows::core::Error) -> Error {
    Error::Os {
        call,
        detail: e.message(),
    }
}
