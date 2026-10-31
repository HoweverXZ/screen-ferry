//! 剪贴板：`Clipboard` 的测试实现。

use std::sync::{Arc, Mutex, MutexGuard};

use ferry_platform::{Clipboard, Result};
use ferry_types::{ClipboardContent, ClipboardFormat};

/// 假的剪贴板：测试用 [`copy`](Self::copy) 模拟用户复制，用 [`content`](Self::content) 观察结果。
#[derive(Clone, Default)]
pub struct MockClipboard {
    state: Arc<Mutex<State>>,
}

#[derive(Default)]
struct State {
    content: ClipboardContent,
    count: u64,
    writes: usize,
}

impl MockClipboard {
    pub fn new() -> Self {
        Self::default()
    }

    /// 模拟本机用户复制了一段内容。
    pub fn copy(&self, content: ClipboardContent) {
        let mut state = self.lock();
        state.content = content;
        state.count += 1;
    }

    /// 当前剪贴板内容。
    pub fn content(&self) -> ClipboardContent {
        self.lock().content.clone()
    }

    /// 被测代码写入剪贴板的次数。
    pub fn writes(&self) -> usize {
        self.lock().writes
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Clipboard for MockClipboard {
    fn formats(&self) -> &'static [ClipboardFormat] {
        &[ClipboardFormat::TEXT, ClipboardFormat::HTML]
    }

    fn change_count(&mut self) -> u64 {
        self.lock().count
    }

    fn read(&mut self) -> Result<ClipboardContent> {
        Ok(self.lock().content.clone())
    }

    fn write(&mut self, content: &ClipboardContent) -> Result<u64> {
        let mut state = self.lock();
        state.content = content.clone();
        state.count += 1;
        state.writes += 1;
        Ok(state.count)
    }
}
