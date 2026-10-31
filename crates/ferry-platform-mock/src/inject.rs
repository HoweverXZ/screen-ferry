//! 输入注入：`InputInject` 的测试实现。

use std::sync::{Arc, Mutex};

use ferry_platform::{InputInject, Result};
use ferry_types::InputEvent;

/// 假的输入注入：记录所有注入的事件。
#[derive(Clone, Default)]
pub struct MockInject {
    events: Arc<Mutex<Vec<InputEvent>>>,
}

impl MockInject {
    pub fn new() -> Self {
        Self::default()
    }

    /// 至今注入过的全部事件。
    pub fn events(&self) -> Vec<InputEvent> {
        self.events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

impl InputInject for MockInject {
    fn inject(&mut self, event: &InputEvent) -> Result<()> {
        self.events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(*event);
        Ok(())
    }
}
