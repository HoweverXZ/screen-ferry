//! 光标在对方屏幕上时藏起本机光标。控制别人（捕获）和被控制（注入）两边共用。
//!
//! 后台程序藏光标要先给本进程的连接打开 `SetsCursorInBackground`（见
//! [`allow_cursor_control_in_background`]），但有两处系统会把光标的控制权抢回去：
//! - 光标停在 Dock 上时，窗口服务器始终让 Dock 管光标，别的程序藏不住；
//! - 全屏应用里光标碰到顶边，菜单栏滑出来，会把已经藏起的光标重新显示出来。
//!
//! 前台应用设置光标的样子时（例如界面刷新时把光标设成箭头），也会把它显示出来：前台应用优先。
//!
//! 所以：
//! 1. 藏之前先把光标挪到主显示器中央，离开 Dock、菜单栏和触发角；
//! 2. 在那个位置放一个极小、几乎透明的置顶窗口，光标底下是本程序的窗口，而不是前台应用的；
//! 3. 藏着的时候隔一会儿再确认一次：先显示一次再藏一次。隐藏由系统计数，这一对调用前后计数不变；
//!    系统只在计数从 0 变成 1 的那一刻真正把光标藏起来，所以要先降到 0 再升回 1，
//!    被抢走的控制权借这一次拿回来。
//!
//! 被控制的一方还要看着光标位置：本机的人自己动了鼠标，光标就重新出现。

use std::cell::RefCell;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use dispatch2::DispatchQueue;
use ferry_types::Point;
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSBackingStoreType, NSColor, NSPanel, NSScreen, NSScreenSaverWindowLevel,
    NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_core_graphics::{
    CGDisplayBounds, CGDisplayHideCursor, CGDisplayShowCursor, CGMainDisplayID,
    CGWarpMouseCursorPosition,
};

use crate::capture::{allow_cursor_control_in_background, disable_local_suppression, live_cursor};

/// 看守线程多久醒一次：看本机的人有没有动鼠标。
const TICK: Duration = Duration::from_millis(50);
/// 藏起之后在这些时刻再确认一次：菜单栏、Dock 的动画在藏起之后才结束。
const EARLY_CHECKS: [Duration; 3] = [
    Duration::from_millis(100),
    Duration::from_millis(300),
    Duration::from_millis(700),
];
/// 之后每隔这么久确认一次。
const RECHECK: Duration = Duration::from_secs(1);
/// 遮挡窗口的边长（点）。只要盖住光标的热点就够了。
const SHIELD_SIZE: f64 = 4.0;
/// 遮挡窗口的不透明度：完全透明的窗口会被系统当成不存在，光标还是落在下面的窗口上。
const SHIELD_ALPHA: f64 = 0.04;

thread_local! {
    /// 主线程上的遮挡窗口，第一次用时创建。
    static SHIELD: RefCell<Option<Retained<NSPanel>>> = const { RefCell::new(None) };
}

pub(crate) struct CursorHider {
    state: Arc<Mutex<State>>,
}

struct State {
    hidden: bool,
    /// 每藏一次、显示一次加一；看守线程发现它变了就退出。
    generation: u64,
}

impl CursorHider {
    pub(crate) fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                hidden: false,
                generation: 0,
            })),
        }
    }

    /// 把光标挪到 `parked`（见 [`park_point`]）再藏起来。已经藏着时什么也不做，返回假。
    ///
    /// `reveal_on_move`：本机的人一动鼠标就重新显示（被控制的一方）。控制别人时光标和鼠标已经脱开，
    /// 光标不会动，不需要看。
    pub(crate) fn hide(&self, parked: Point, reveal_on_move: bool) -> bool {
        let mut state = lock(&self.state);
        if state.hidden {
            return false;
        }
        allow_cursor_control_in_background();
        // 程序挪光标之后，系统默认压住本机真实的键鼠一会儿。
        disable_local_suppression();
        let park = CGPoint::new(f64::from(parked.x), f64::from(parked.y));
        // 挪不动、藏不住都不影响控制，只是本机屏幕上还看得到光标。
        let _ = CGWarpMouseCursorPosition(park);
        let _ = CGDisplayHideCursor(CGMainDisplayID());
        shield(Some(park));
        state.hidden = true;
        state.generation += 1;
        let generation = state.generation;
        drop(state);

        let shared = Arc::clone(&self.state);
        // 起不了线程就只是不会再确认、不会自动重新出现。
        let _ = thread::Builder::new()
            .name("ferry-cursor".into())
            .spawn(move || watch(&shared, generation, parked, reveal_on_move));
        true
    }

    pub(crate) fn show(&self) {
        let mut state = lock(&self.state);
        state.generation += 1;
        if state.hidden {
            state.hidden = false;
            shield(None);
            let _ = CGDisplayShowCursor(CGMainDisplayID());
        }
    }
}

/// 藏光标时把它停在哪里：主显示器的中央，离开 Dock、菜单栏和触发角。
pub(crate) fn park_point() -> Point {
    let bounds = CGDisplayBounds(CGMainDisplayID());
    Point::new(
        (bounds.origin.x + bounds.size.width / 2.0).floor() as i32,
        (bounds.origin.y + bounds.size.height / 2.0).floor() as i32,
    )
}

impl Drop for CursorHider {
    fn drop(&mut self) {
        self.show();
    }
}

/// 看守线程：按时再确认一次藏着；被控制的一方看到本机的人动了鼠标就显示。
fn watch(state: &Mutex<State>, generation: u64, parked: Point, reveal_on_move: bool) {
    let start = Instant::now();
    let mut early = EARLY_CHECKS.iter().peekable();
    let mut next_recheck = None;
    loop {
        thread::sleep(TICK);
        let mut state = lock(state);
        if !state.hidden || state.generation != generation {
            return;
        }
        if reveal_on_move && live_cursor().is_some_and(|p| p != parked) {
            state.hidden = false;
            shield(None);
            let _ = CGDisplayShowCursor(CGMainDisplayID());
            return;
        }
        let elapsed = start.elapsed();
        let due = match (early.peek(), next_recheck) {
            (Some(&&at), _) if elapsed >= at => {
                early.next();
                true
            }
            (None, None) => {
                next_recheck = Some(elapsed + RECHECK);
                false
            }
            (None, Some(at)) if elapsed >= at => {
                next_recheck = Some(elapsed + RECHECK);
                true
            }
            _ => false,
        };
        if due {
            // 先显示再藏：计数 1 → 0 → 1，从 0 到 1 的那一下才会真正藏起来。
            // 两次调用紧挨着，中间来不及画出一帧光标。
            let _ = CGDisplayShowCursor(CGMainDisplayID());
            let _ = CGDisplayHideCursor(CGMainDisplayID());
        }
    }
}

/// 在主线程上把遮挡窗口放到 `park`（桌面坐标，左上角原点），或者收起它。
/// 主线程没有在跑事件循环时（命令行模式）什么也不会发生，只是少了这一层。
fn shield(park: Option<CGPoint>) {
    DispatchQueue::main().exec_async(move || {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        SHIELD.with(|cell| {
            let mut cell = cell.borrow_mut();
            let Some(park) = park else {
                if let Some(panel) = cell.as_ref() {
                    panel.orderOut(None);
                }
                return;
            };
            let panel = cell.get_or_insert_with(|| create_shield(mtm));
            // 桌面坐标 → Cocoa 坐标（主屏幕左下角原点）。
            let height = NSScreen::screens(mtm)
                .firstObject()
                .map_or(0.0, |screen| screen.frame().size.height);
            let half = SHIELD_SIZE / 2.0;
            let rect = CGRect::new(
                CGPoint::new(park.x - half, height - park.y - half),
                CGSize::new(SHIELD_SIZE, SHIELD_SIZE),
            );
            panel.setFrame_display(rect, false);
            panel.orderFrontRegardless();
        });
    });
}

fn create_shield(mtm: MainThreadMarker) -> Retained<NSPanel> {
    let rect = CGRect::new(
        CGPoint::new(0.0, 0.0),
        CGSize::new(SHIELD_SIZE, SHIELD_SIZE),
    );
    let style = NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel;
    let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
        NSPanel::alloc(mtm),
        rect,
        style,
        NSBackingStoreType::Buffered,
        false,
    );
    // SAFETY: 窗口由本模块持有，关闭时不能被释放。
    unsafe { panel.setReleasedWhenClosed(false) };
    // 比 Dock、菜单栏和普通的置顶窗口都高。
    panel.setLevel(NSScreenSaverWindowLevel);
    panel.setOpaque(false);
    panel.setBackgroundColor(Some(&NSColor::colorWithWhite_alpha(0.0, SHIELD_ALPHA)));
    panel.setHasShadow(false);
    // 明确设为不忽略：整个窗口都算光标底下的窗口。
    panel.setIgnoresMouseEvents(false);
    panel.setHidesOnDeactivate(false);
    panel.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::Stationary
            | NSWindowCollectionBehavior::IgnoresCycle,
    );
    panel
}

fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    state.lock().unwrap_or_else(|e| e.into_inner())
}
