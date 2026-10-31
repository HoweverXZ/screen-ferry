//! 主线程事件循环：`EventLoop` 的测试实现。任务排进队列，测试里用 [`MockEventLoop::run_pending`] 执行。

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use ferry_platform::{Dispatcher, EventLoop, MainTask};

#[derive(Default)]
pub struct MockEventLoop {
    queue: Arc<Mutex<VecDeque<MainTask>>>,
    reopen: Option<Box<dyn Fn()>>,
}

impl MockEventLoop {
    pub fn new() -> Self {
        Self::default()
    }

    /// 执行目前排着的全部任务，返回执行了几个。
    pub fn run_pending(&self) -> usize {
        let tasks: Vec<MainTask> = self.queue.lock().unwrap().drain(..).collect();
        let count = tasks.len();
        for task in tasks {
            task();
        }
        count
    }

    /// 模拟用户再次打开应用。
    pub fn reopen(&self) {
        if let Some(handler) = &self.reopen {
            handler();
        }
    }
}

impl EventLoop for MockEventLoop {
    fn dispatcher(&self) -> Dispatcher {
        let queue = self.queue.clone();
        Dispatcher::new(move |task| queue.lock().unwrap().push_back(task))
    }

    fn on_reopen(&mut self, handler: Box<dyn Fn()>) {
        self.reopen = Some(handler);
    }

    fn run(self) -> ! {
        loop {
            self.run_pending();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}
