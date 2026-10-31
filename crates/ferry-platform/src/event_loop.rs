//! 主线程事件循环：定义 `EventLoop` trait。
//!
//! 托盘图标这类界面元素只能在主线程上创建和修改，系统的事件循环也必须跑在主线程上。
//! 后台把引擎和本机通信放在别的线程，需要改托盘时用 [`Dispatcher`] 把任务交给主线程。

use std::sync::Arc;

/// 交给主线程执行的任务。
pub type MainTask = Box<dyn FnOnce() + Send + 'static>;

/// 把任务交给主线程执行。可以克隆，在任意线程调用。
#[derive(Clone)]
pub struct Dispatcher(Arc<dyn Fn(MainTask) + Send + Sync>);

impl Dispatcher {
    /// `post` 负责把任务排进主线程的事件循环。
    pub fn new(post: impl Fn(MainTask) + Send + Sync + 'static) -> Self {
        Self(Arc::new(post))
    }

    /// 让主线程稍后执行 `task`，立即返回。
    pub fn run_on_main(&self, task: impl FnOnce() + Send + 'static) {
        (self.0)(Box::new(task));
    }
}

/// 主线程的事件循环。只能在主线程上创建。
pub trait EventLoop {
    fn dispatcher(&self) -> Dispatcher;

    /// 用户再次打开已在运行的应用时调用，例如在 macOS 的访达或程序坞里再点一次。
    fn on_reopen(&mut self, handler: Box<dyn Fn()>);

    /// 在主线程上运行事件循环，直到进程退出。
    fn run(self) -> !;
}
