//! 光标在对方屏幕上时隐藏本机光标。
//!
//! 隐藏与显示由系统计数，必须一一对应，所以用一个标志记下现在是否藏着。
//! 藏起来以后每隔一会儿看一眼光标位置：本机的人自己动了鼠标，光标就重新出现。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use objc2_core_graphics::{CGDisplayHideCursor, CGDisplayShowCursor, CGMainDisplayID};

use crate::capture::{allow_cursor_control_in_background, live_cursor};

/// 藏着的时候多久看一次光标位置。
const POLL: Duration = Duration::from_millis(50);

pub(super) struct CursorHider {
    hidden: Arc<AtomicBool>,
    /// 每藏一次、显示一次加一；看守线程发现它变了就退出。
    generation: Arc<AtomicU64>,
}

impl CursorHider {
    pub(super) fn new() -> Self {
        Self {
            hidden: Arc::new(AtomicBool::new(false)),
            generation: Arc::new(AtomicU64::new(0)),
        }
    }

    pub(super) fn hide(&self) {
        if self.hidden.swap(true, Ordering::SeqCst) {
            return;
        }
        allow_cursor_control_in_background();
        // 隐藏失败不影响控制，只是本机屏幕上还看得到光标。
        let _ = CGDisplayHideCursor(CGMainDisplayID());
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let (hidden, current) = (Arc::clone(&self.hidden), Arc::clone(&self.generation));
        let parked = live_cursor();
        let watch = move || {
            loop {
                thread::sleep(POLL);
                if current.load(Ordering::SeqCst) != generation {
                    return;
                }
                if live_cursor() != parked {
                    if hidden.swap(false, Ordering::SeqCst) {
                        let _ = CGDisplayShowCursor(CGMainDisplayID());
                    }
                    return;
                }
            }
        };
        // 起不了线程就只是不会自动重新出现。
        let _ = thread::Builder::new()
            .name("ferry-cursor".into())
            .spawn(watch);
    }

    pub(super) fn show(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        if self.hidden.swap(false, Ordering::SeqCst) {
            let _ = CGDisplayShowCursor(CGMainDisplayID());
        }
    }
}

impl Drop for CursorHider {
    fn drop(&mut self) {
        self.show();
    }
}
