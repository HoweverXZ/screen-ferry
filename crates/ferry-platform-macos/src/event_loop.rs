//! 主线程事件循环：`EventLoop` 的 macOS 实现。
//!
//! 运行 NSApplication，不显示在程序坞里，只有菜单栏图标。其他线程的任务经 GCD 主队列交给主线程；
//! 用户在访达或启动台里再次打开已在运行的应用时，系统发来 reopen，转给 [`EventLoop::on_reopen`]。

use std::cell::RefCell;

use dispatch2::DispatchQueue;
use ferry_platform::{Dispatcher, Error, EventLoop, Result};
use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate};

struct Ivars {
    reopen: RefCell<Option<Box<dyn Fn()>>>,
}

define_class!(
    // SAFETY: NSObject 没有子类化要求；AppDelegate 没有实现 Drop。
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "ScreenFerryAppDelegate"]
    #[ivars = Ivars]
    struct AppDelegate;

    unsafe impl NSObjectProtocol for AppDelegate {}

    unsafe impl NSApplicationDelegate for AppDelegate {
        #[unsafe(method(applicationShouldHandleReopen:hasVisibleWindows:))]
        fn should_handle_reopen(&self, _app: &NSApplication, _visible: bool) -> bool {
            if let Some(handler) = self.ivars().reopen.borrow().as_ref() {
                handler();
            }
            true
        }
    }
);

impl AppDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(Ivars {
            reopen: RefCell::new(None),
        });
        // SAFETY: NSObject 的 init 总是可以调用。
        unsafe { msg_send![super(this), init] }
    }
}

pub struct MacEventLoop {
    app: Retained<NSApplication>,
    /// NSApplication 只弱引用代理，这里持有它。
    delegate: Retained<AppDelegate>,
}

impl MacEventLoop {
    /// 只能在主线程上调用。
    pub fn new() -> Result<Self> {
        let mtm =
            MainThreadMarker::new().ok_or(Error::Unsupported("事件循环只能在主线程上创建"))?;
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
        let delegate = AppDelegate::new(mtm);
        app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        Ok(Self { app, delegate })
    }
}

impl EventLoop for MacEventLoop {
    fn dispatcher(&self) -> Dispatcher {
        Dispatcher::new(|task| DispatchQueue::main().exec_async(task))
    }

    fn on_reopen(&mut self, handler: Box<dyn Fn()>) {
        *self.delegate.ivars().reopen.borrow_mut() = Some(handler);
    }

    fn run(self) -> ! {
        self.app.run();
        drop(self.delegate);
        std::process::exit(0)
    }
}
