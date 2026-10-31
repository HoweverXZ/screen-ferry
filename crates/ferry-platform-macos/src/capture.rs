//! 输入捕获：`InputCapture` 的 macOS 实现。
//!
//! 用 CGEventTap 在会话层拦截鼠标事件，回调运行在专用线程的 CFRunLoop 上。抓取期间：
//! - 光标与鼠标解耦，光标停在原地，事件里仍带有相对位移；
//! - 本机光标隐藏起来，免得对方屏幕上的光标在动、本机屏幕边缘还停着一个；
//! - 事件被吞掉，本机程序收不到。
//!
//! 键盘：光标在本机时只记录哪些键按着，不交给 sink；抓取期间转交 sink 并吞掉。
//! 抓取前就按着的键，之后的连发和松开仍交给本机，免得本机以为它一直按着。
//!
//! 媒体键（音量、播放）不是普通按键，而是 `NX_SYSDEFINED` 系统事件，按同样的规则处理。
//! 不认识的系统事件（亮度、键盘背光、电源键等）一律交给本机。

use std::collections::BTreeSet;
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr::{self, NonNull};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Once, mpsc};
use std::thread::{self, JoinHandle};

use ferry_platform::{CaptureEvent, CaptureSink, Error, InputCapture, Result};
use ferry_types::{KeyAction, KeyCode, LockKeys, MouseButton, Point, WHEEL_NOTCH};
use objc2::rc::autoreleasepool;
use objc2_app_kit::NSEvent;
use objc2_core_foundation::{
    CFMachPort, CFRetained, CFRunLoop, CGPoint, kCFRunLoopCommonModes, kCFRunLoopDefaultMode,
};
use objc2_core_graphics::{
    CGAssociateMouseAndMouseCursorPosition, CGDisplayHideCursor, CGDisplayShowCursor, CGEvent,
    CGEventField, CGEventFlags, CGEventSource, CGEventSourceStateID, CGEventTapLocation,
    CGEventTapOptions, CGEventTapPlacement, CGEventTapProxy, CGEventType, CGMainDisplayID,
    CGWarpMouseCursorPosition,
};
use objc2_foundation::NSString;

use crate::display::check;
use crate::keymap::{hid_code, media_key, modifier_flag};
use crate::permission::ACCESSIBILITY;

/// `NX_SYSDEFINED`：媒体键等系统事件。CGEventType 没有为它命名。
const SYSTEM_DEFINED: CGEventType = CGEventType(14);
/// 系统事件中“辅助控制键”（媒体键）的子类型 `NX_SUBTYPE_AUX_CONTROL_BUTTONS`。
const AUX_CONTROL_BUTTONS: i16 = 8;

/// 拦截的事件类型。
const EVENTS: [CGEventType; 15] = [
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
    CGEventType::KeyDown,
    CGEventType::KeyUp,
    CGEventType::FlagsChanged,
    SYSTEM_DEFINED,
];

/// 捕获线程每隔这么久检查一次是否该退出（同时也会被立即唤醒）。
const POLL_SECONDS: f64 = 0.5;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    /// 本进程与窗口服务器之间的连接。系统私有接口，没有公开的头文件。
    fn _CGSDefaultConnection() -> i32;
    /// 设置这条连接的属性。系统私有接口。
    fn CGSSetConnectionProperty(
        connection: i32,
        target: i32,
        key: *const c_void,
        value: *const c_void,
    ) -> i32;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFBooleanTrue: *const c_void;
}

/// 后台程序默认改不了光标的显示：窗口服务器只听前台应用的。给本进程的连接打开
/// `SetsCursorInBackground` 之后，隐藏光标才对后台程序生效。整个进程只需做一次。
fn allow_cursor_control_in_background() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let key = NSString::from_str("SetsCursorInBackground");
        // SAFETY: NSString 与 CFString 可以互相转换；键和值在调用期间都有效。
        unsafe {
            let connection = _CGSDefaultConnection();
            CGSSetConnectionProperty(
                connection,
                connection,
                objc2::rc::Retained::as_ptr(&key).cast(),
                kCFBooleanTrue,
            );
        }
    });
}

pub struct MacCapture {
    grabbed: Arc<AtomicBool>,
    /// 本机光标是否被隐藏。隐藏与显示由系统计数，必须一一对应，所以记下来。
    cursor_hidden: AtomicBool,
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
    /// 交给本机处理、尚未松开的键。
    local_keys: BTreeSet<KeyCode>,
}

impl MacCapture {
    pub fn new() -> Self {
        Self {
            grabbed: Arc::new(AtomicBool::new(false)),
            cursor_hidden: AtomicBool::new(false),
            worker: None,
        }
    }

    fn hide_cursor(&self) {
        if self.cursor_hidden.swap(true, Ordering::SeqCst) {
            return;
        }
        allow_cursor_control_in_background();
        // 隐藏失败不影响切换，只是本机屏幕上还看得到光标。
        let _ = CGDisplayHideCursor(CGMainDisplayID());
    }

    fn show_cursor(&self) {
        if self.cursor_hidden.swap(false, Ordering::SeqCst) {
            let _ = CGDisplayShowCursor(CGMainDisplayID());
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
        self.hide_cursor();
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
        // 光标已经放到回来的位置，再让它出现。
        self.show_cursor();
        warped.and(associated)
    }

    fn lock_keys(&self) -> LockKeys {
        // macOS 只有 Caps Lock。
        let flags = CGEventSource::flags_state(CGEventSourceStateID::CombinedSessionState);
        if flags.contains(CGEventFlags::MaskAlphaShift) {
            LockKeys::CAPS
        } else {
            LockKeys::empty()
        }
    }
}

impl Drop for MacCapture {
    fn drop(&mut self) {
        if self.grabbed.swap(false, Ordering::SeqCst) {
            let _ = CGAssociateMouseAndMouseCursorPosition(true);
        }
        self.show_cursor();
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
        local_keys: BTreeSet::new(),
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

    let grabbed = state.grabbed.load(Ordering::Relaxed);
    // 抓取期间吞掉事件，否则交给本机。
    let verdict = if grabbed {
        ptr::null_mut()
    } else {
        event.as_ptr()
    };
    // SAFETY: 系统保证回调期间事件有效。
    let Some(captured) = translate(ty, unsafe { event.as_ref() }) else {
        // 不认识的系统事件（亮度、电源键等）始终交给本机，不能吞掉。
        return if ty == SYSTEM_DEFINED {
            event.as_ptr()
        } else {
            verdict
        };
    };

    if let CaptureEvent::Key { code, action } = captured {
        if !grabbed {
            // 光标在本机：按键照常交给本机，只记下哪些键按着。Caps Lock 没有松开事件，不记录。
            if code != KeyCode::CAPS_LOCK {
                match action {
                    KeyAction::Up => state.local_keys.remove(&code),
                    KeyAction::Down | KeyAction::Repeat => state.local_keys.insert(code),
                };
            }
            return event.as_ptr();
        }
        if state.local_keys.contains(&code) {
            // 抓取前就按着的键：连发和松开都交给本机。
            if action == KeyAction::Up {
                state.local_keys.remove(&code);
            }
            return event.as_ptr();
        }
    }

    emit(state, captured);
    if captured
        == (CaptureEvent::Key {
            code: KeyCode::CAPS_LOCK,
            action: KeyAction::Down,
        })
    {
        // Caps Lock 每按一次只报告一次，转发成一次完整的按下与松开。
        emit(
            state,
            CaptureEvent::Key {
                code: KeyCode::CAPS_LOCK,
                action: KeyAction::Up,
            },
        );
    }
    verdict
}

/// 把事件交给 sink。panic 不能穿过系统框架，这里截住。
fn emit(state: &mut TapState, event: CaptureEvent) {
    let _ = catch_unwind(AssertUnwindSafe(|| (state.sink)(event)));
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
        CGEventType::KeyDown | CGEventType::KeyUp => {
            let code = hid_code(int(CGEventField::KeyboardEventKeycode) as u16)?;
            let action = if ty == CGEventType::KeyUp {
                KeyAction::Up
            } else if int(CGEventField::KeyboardEventAutorepeat) != 0 {
                KeyAction::Repeat
            } else {
                KeyAction::Down
            };
            Some(CaptureEvent::Key { code, action })
        }
        CGEventType::FlagsChanged => {
            let keycode = int(CGEventField::KeyboardEventKeycode) as u16;
            let code = hid_code(keycode)?;
            if code == KeyCode::CAPS_LOCK {
                return Some(CaptureEvent::Key {
                    code,
                    action: KeyAction::Down,
                });
            }
            // 修饰键只报告“状态变了”，按设备相关的标志位判断是按下还是松开。
            let held = CGEvent::flags(Some(event)).0 & modifier_flag(keycode)? != 0;
            let action = if held { KeyAction::Down } else { KeyAction::Up };
            Some(CaptureEvent::Key { code, action })
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
        SYSTEM_DEFINED => media(event),
        _ => None,
    }
}

/// 解出媒体键。键码和状态打包在 NSEvent 的 `data1` 里：
/// 高 16 位是 `NX_KEYTYPE_*`，低 16 位中高 8 位是状态（0xA 按下、0xB 松开），最低位表示连发。
fn media(event: &CGEvent) -> Option<CaptureEvent> {
    // 捕获线程没有现成的自动释放池，NSEvent 是自动释放的对象。
    let data = autoreleasepool(|_| {
        let ns = NSEvent::eventWithCGEvent(event)?;
        (ns.subtype().0 == AUX_CONTROL_BUTTONS).then(|| ns.data1() as u64)
    })?;
    let code = media_key(((data >> 16) & 0xFFFF) as u32)?;
    let repeat = data & 1 != 0;
    let action = match (data >> 8) & 0xFF {
        0x0A if repeat => KeyAction::Repeat,
        0x0A => KeyAction::Down,
        0x0B => KeyAction::Up,
        _ => return None,
    };
    Some(CaptureEvent::Key { code, action })
}
