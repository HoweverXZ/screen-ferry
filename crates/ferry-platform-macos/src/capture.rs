//! 输入捕获：`InputCapture` 的 macOS 实现。
//!
//! 用 CGEventTap 在会话层拦截鼠标事件，回调运行在专用线程的 CFRunLoop 上。抓取期间：
//! - 光标与鼠标解耦，光标停在原地，事件里仍带有相对位移；
//! - 事件被吞掉，本机程序收不到。
//!
//! 键盘事件暂不拦截，原型阶段键盘仍作用于本机。

use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr::{self, NonNull};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use ferry_platform::{CaptureEvent, CaptureSink, Error, InputCapture, Result};
use ferry_types::{MouseButton, Point, WHEEL_NOTCH};
use objc2_core_foundation::{
    CFMachPort, CFRetained, CFRunLoop, CGPoint, kCFRunLoopCommonModes, kCFRunLoopDefaultMode,
};
use objc2_core_graphics::{
    CGAssociateMouseAndMouseCursorPosition, CGEvent, CGEventField, CGEventTapLocation,
    CGEventTapOptions, CGEventTapPlacement, CGEventTapProxy, CGEventType,
    CGWarpMouseCursorPosition,
};

use crate::display::check;
use crate::permission::ACCESSIBILITY;

/// 拦截的事件类型。
const EVENTS: [CGEventType; 11] = [
    CGEventType::MouseMoved,
    CGEventType::LeftMouseDown,
    CGEventType::LeftMouseUp,
    CGEventType::LeftMouseDragged,
    CGEventType::RightMouseDown,
    CGEventType::RightMouseUp,
    CGEventType::RightMouseDragged,
    CGEventType::OtherMouseDown,
    CGEventType::OtherMouseUp,
    CGEventType::OtherMouseDragged,
    CGEventType::ScrollWheel,
];

/// 捕获线程每隔这么久检查一次是否该退出（同时也会被立即唤醒）。
const POLL_SECONDS: f64 = 0.5;

pub struct MacCapture {
    grabbed: Arc<AtomicBool>,
    worker: Option<Worker>,
}

struct Worker {
    stop: Arc<AtomicBool>,
    run_loop: RunLoop,
    thread: JoinHandle<()>,
}

/// 可以交给其他线程去停止的 CFRunLoop。
struct RunLoop(CFRetained<CFRunLoop>);

// SAFETY: 其他线程只会对它调用 `CFRunLoopStop`，苹果文档说明该函数可以跨线程调用。
unsafe impl Send for RunLoop {}

/// 捕获线程独占的状态，以指针形式交给事件回调。
struct TapState {
    sink: CaptureSink,
    grabbed: Arc<AtomicBool>,
    tap: Option<CFRetained<CFMachPort>>,
}

impl MacCapture {
    pub fn new() -> Self {
        Self {
            grabbed: Arc::new(AtomicBool::new(false)),
            worker: None,
        }
    }
}

impl Default for MacCapture {
    fn default() -> Self {
        Self::new()
    }
}

impl InputCapture for MacCapture {
    fn start(&mut self, sink: CaptureSink) -> Result<()> {
        if self.worker.is_some() {
            return Err(Error::Os {
                call: "MacCapture::start",
                detail: "已经在捕获".into(),
            });
        }
        let grabbed = Arc::clone(&self.grabbed);
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let (ready_tx, ready_rx) = mpsc::channel();

        let thread = thread::Builder::new()
            .name("ferry-capture".into())
            .spawn(move || run(grabbed, thread_stop, sink, ready_tx))
            .map_err(|e| Error::Os {
                call: "thread::spawn",
                detail: e.to_string(),
            })?;

        match ready_rx.recv() {
            Ok(Ok(run_loop)) => {
                self.worker = Some(Worker {
                    stop,
                    run_loop,
                    thread,
                });
                Ok(())
            }
            Ok(Err(e)) => {
                let _ = thread.join();
                Err(e)
            }
            Err(_) => {
                let _ = thread.join();
                Err(Error::Os {
                    call: "CGEventTapCreate",
                    detail: "捕获线程意外退出".into(),
                })
            }
        }
    }

    fn grab(&self) -> Result<()> {
        check(
            CGAssociateMouseAndMouseCursorPosition(false),
            "CGAssociateMouseAndMouseCursorPosition",
        )?;
        self.grabbed.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn release(&self, at: Point) -> Result<()> {
        self.grabbed.store(false, Ordering::SeqCst);
        let to = CGPoint::new(f64::from(at.x), f64::from(at.y));
        let warped = check(CGWarpMouseCursorPosition(to), "CGWarpMouseCursorPosition");
        // 无论光标是否移动成功都要重新关联，否则本机鼠标会一直失灵。
        // 紧跟在 warp 之后关联，还能避免系统让光标停顿约 0.25 秒。
        let associated = check(
            CGAssociateMouseAndMouseCursorPosition(true),
            "CGAssociateMouseAndMouseCursorPosition",
        );
        warped.and(associated)
    }
}

impl Drop for MacCapture {
    fn drop(&mut self) {
        if self.grabbed.swap(false, Ordering::SeqCst) {
            let _ = CGAssociateMouseAndMouseCursorPosition(true);
        }
        if let Some(worker) = self.worker.take() {
            worker.stop.store(true, Ordering::SeqCst);
            worker.run_loop.0.stop();
            let _ = worker.thread.join();
        }
    }
}

/// 捕获线程主体：建立事件 tap，运行 run loop 直到被要求退出。
fn run(
    grabbed: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    sink: CaptureSink,
    ready: mpsc::Sender<Result<RunLoop>>,
) {
    let state = Box::into_raw(Box::new(TapState {
        sink,
        grabbed,
        tap: None,
    }));
    let mask = EVENTS.iter().fold(0u64, |mask, ty| mask | (1 << ty.0));

    // SAFETY: `on_event` 符合 CGEventTapCallBack 的约定；`state` 在 tap 存活期间一直有效，
    // 并且只在本线程（run loop 回调中）访问。
    let tap = unsafe {
        CGEvent::tap_create(
            CGEventTapLocation::SessionEventTap,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::Default,
            mask,
            Some(on_event),
            state.cast(),
        )
    };

    let started = tap
        .ok_or(Error::PermissionDenied(ACCESSIBILITY))
        .and_then(|tap| {
            let source = CFMachPort::new_run_loop_source(None, Some(&tap), 0).ok_or(Error::Os {
                call: "CFMachPortCreateRunLoopSource",
                detail: "返回空".into(),
            })?;
            let run_loop = CFRunLoop::current().ok_or(Error::Os {
                call: "CFRunLoopGetCurrent",
                detail: "返回空".into(),
            })?;
            // SAFETY: 读取 CoreFoundation 导出的常量。
            run_loop.add_source(Some(&source), unsafe { kCFRunLoopCommonModes });
            CGEvent::tap_enable(&tap, true);
            // SAFETY: run loop 尚未运行，回调不会同时访问 `state`。
            unsafe { (*state).tap = Some(tap) };
            Ok(run_loop)
        });

    match started {
        Ok(run_loop) => {
            if ready.send(Ok(RunLoop(run_loop))).is_ok() {
                while !stop.load(Ordering::SeqCst) {
                    // SAFETY: 读取 CoreFoundation 导出的常量。
                    CFRunLoop::run_in_mode(unsafe { kCFRunLoopDefaultMode }, POLL_SECONDS, false);
                }
            }
        }
        Err(e) => {
            let _ = ready.send(Err(e));
        }
    }

    // SAFETY: run loop 已退出，回调不会再访问 `state`；这里收回 `Box::into_raw` 的所有权。
    let state = unsafe { Box::from_raw(state) };
    if let Some(tap) = &state.tap {
        CGEvent::tap_enable(tap, false);
    }
}

/// CGEventTap 回调。
unsafe extern "C-unwind" fn on_event(
    _proxy: CGEventTapProxy,
    ty: CGEventType,
    event: NonNull<CGEvent>,
    user_info: *mut c_void,
) -> *mut CGEvent {
    // SAFETY: `user_info` 是 `run` 中创建的 `TapState`，只在本线程访问。
    let state = unsafe { &mut *user_info.cast::<TapState>() };

    // 回调太慢或用户操作时，系统会关掉 tap，需要重新打开。
    if ty == CGEventType::TapDisabledByTimeout || ty == CGEventType::TapDisabledByUserInput {
        if let Some(tap) = &state.tap {
            CGEvent::tap_enable(tap, true);
        }
        return event.as_ptr();
    }

    // SAFETY: 系统保证回调期间事件有效。
    if let Some(captured) = translate(ty, unsafe { event.as_ref() }) {
        // panic 不能穿过系统框架，这里截住。
        let _ = catch_unwind(AssertUnwindSafe(|| (state.sink)(captured)));
    }

    if state.grabbed.load(Ordering::Relaxed) {
        ptr::null_mut()
    } else {
        event.as_ptr()
    }
}

fn translate(ty: CGEventType, event: &CGEvent) -> Option<CaptureEvent> {
    let int = |field| CGEvent::integer_value_field(Some(event), field);
    let button = |button, pressed| Some(CaptureEvent::Button { button, pressed });

    match ty {
        CGEventType::MouseMoved
        | CGEventType::LeftMouseDragged
        | CGEventType::RightMouseDragged
        | CGEventType::OtherMouseDragged => {
            let p = CGEvent::location(Some(event));
            Some(CaptureEvent::Pointer {
                position: Point::new(p.x.floor() as i32, p.y.floor() as i32),
                dx: int(CGEventField::MouseEventDeltaX) as i32,
                dy: int(CGEventField::MouseEventDeltaY) as i32,
            })
        }
        CGEventType::LeftMouseDown => button(MouseButton::LEFT, true),
        CGEventType::LeftMouseUp => button(MouseButton::LEFT, false),
        CGEventType::RightMouseDown => button(MouseButton::RIGHT, true),
        CGEventType::RightMouseUp => button(MouseButton::RIGHT, false),
        CGEventType::OtherMouseDown | CGEventType::OtherMouseUp => {
            // macOS 从 0 开始编号（0 左、1 右、2 中），协议从 1 开始。
            let number = int(CGEventField::MouseEventButtonNumber).clamp(0, 254) as u8 + 1;
            button(MouseButton::new(number)?, ty == CGEventType::OtherMouseDown)
        }
        CGEventType::ScrollWheel => {
            let lines = |field| CGEvent::double_value_field(Some(event), field);
            let notch = f64::from(WHEEL_NOTCH);
            // Axis1 为正是向上；Axis2 为正是向左，协议约定向右为正，所以取反。
            let dy =
                (lines(CGEventField::ScrollWheelEventFixedPtDeltaAxis1) * notch).round() as i32;
            let dx =
                -(lines(CGEventField::ScrollWheelEventFixedPtDeltaAxis2) * notch).round() as i32;
            (dx != 0 || dy != 0).then_some(CaptureEvent::Wheel { dx, dy })
        }
        _ => None,
    }
}
