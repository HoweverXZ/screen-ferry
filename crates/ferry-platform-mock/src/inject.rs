//! 输入注入：`InputInject` 的测试实现。

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use ferry_platform::{Error, InputInject, Result};
use ferry_types::{InputEvent, LockKeys};

/// 假的输入注入：记录所有注入的事件和锁定键同步。
#[derive(Clone, Default)]
pub struct MockInject {
    state: Arc<Mutex<State>>,
    /// 接下来还要失败几次，见 [`MockInject::fail_cursor`]。
    failures: Arc<AtomicUsize>,
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
    ///
    /// 失败的请求也在里面：被测的代码要能看出它试过几次。
    pub fn cursor_hidden(&self) -> Vec<bool> {
        self.lock().cursor_hidden.clone()
    }

    /// 接下来 `times` 次隐藏或显示本机光标的请求都失败，模拟平台那边没做成。
    pub fn fail_cursor(&self, times: usize) {
        self.failures.store(times, Ordering::SeqCst);
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
        let left = self.failures.load(Ordering::SeqCst);
        if left > 0 {
            self.failures.store(left - 1, Ordering::SeqCst);
            return Err(Error::Unsupported("测试：藏起或显示本机光标失败"));
        }
        Ok(())
    }
}
