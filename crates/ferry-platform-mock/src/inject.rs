//! 输入注入：`InputInject` 的测试实现。

use std::sync::{Arc, Mutex, MutexGuard};

use ferry_platform::{InputInject, Result};
use ferry_types::{InputEvent, LockKeys};

/// 假的输入注入：记录所有注入的事件和锁定键同步。
#[derive(Clone, Default)]
pub struct MockInject {
    state: Arc<Mutex<State>>,
}

#[derive(Default)]
struct State {
    events: Vec<InputEvent>,
    synced_locks: Vec<LockKeys>,
    cursor_hidden: Vec<bool>,
}

impl MockInject {
    pub fn new() -> Self {
        Self::default()
    }

    /// 至今注入过的全部事件。
    pub fn events(&self) -> Vec<InputEvent> {
        self.lock().events.clone()
    }

    /// 每次同步锁定键时要求的状态（只含被同步的那些锁定键）。
    pub fn synced_locks(&self) -> Vec<LockKeys> {
        self.lock().synced_locks.clone()
    }

    /// 每次隐藏（`true`）或显示（`false`）本机光标的请求，按先后排列。
    pub fn cursor_hidden(&self) -> Vec<bool> {
        self.lock().cursor_hidden.clone()
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl InputInject for MockInject {
    fn inject(&mut self, event: &InputEvent) -> Result<()> {
        self.lock().events.push(*event);
        Ok(())
    }

    fn sync_locks(&mut self, wanted: LockKeys, mask: LockKeys) -> Result<()> {
        self.lock().synced_locks.push(wanted & mask);
        Ok(())
    }

    fn set_cursor_hidden(&mut self, hidden: bool) -> Result<()> {
        self.lock().cursor_hidden.push(hidden);
        Ok(())
    }
}
