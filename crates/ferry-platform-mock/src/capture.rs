//! 输入捕获：`InputCapture` 的测试实现。

use std::sync::{Arc, Mutex, MutexGuard};

use ferry_platform::{CaptureEvent, CaptureSink, InputCapture, Result};
use ferry_types::Point;

/// 假的输入捕获：测试用 [`emit`](Self::emit) 模拟本机输入。
#[derive(Clone, Default)]
pub struct MockCapture {
    state: Arc<Mutex<State>>,
}

#[derive(Default)]
struct State {
    sink: Option<CaptureSink>,
    grabbed: bool,
    last_release: Option<Point>,
}

impl MockCapture {
    pub fn new() -> Self {
        Self::default()
    }

    /// 模拟一个本机输入。尚未开始捕获时丢弃。
    pub fn emit(&self, event: CaptureEvent) {
        if let Some(sink) = self.lock().sink.as_mut() {
            sink(event);
        }
    }

    pub fn is_grabbed(&self) -> bool {
        self.lock().grabbed
    }

    /// 最近一次结束抓取时光标被放到的位置。
    pub fn last_release(&self) -> Option<Point> {
        self.lock().last_release
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl InputCapture for MockCapture {
    fn start(&mut self, sink: CaptureSink) -> Result<()> {
        self.lock().sink = Some(sink);
        Ok(())
    }

    fn grab(&self) -> Result<()> {
        self.lock().grabbed = true;
        Ok(())
    }

    fn release(&self, at: Point) -> Result<()> {
        let mut state = self.lock();
        state.grabbed = false;
        state.last_release = Some(at);
        Ok(())
    }
}
