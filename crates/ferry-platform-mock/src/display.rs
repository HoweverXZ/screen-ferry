//! 显示器信息：`Displays` 的测试实现。

use std::sync::{Arc, Mutex, MutexGuard};

use ferry_platform::{Displays, Result};
use ferry_types::Display;

/// 假的显示器列表。克隆出的副本共享同一份列表，测试可以中途 [`set`](Self::set) 模拟改分辨率。
#[derive(Clone, Debug, Default)]
pub struct MockDisplays(Arc<Mutex<Vec<Display>>>);

impl MockDisplays {
    pub fn new(displays: Vec<Display>) -> Self {
        Self(Arc::new(Mutex::new(displays)))
    }

    /// 换成新的显示器列表。
    pub fn set(&self, displays: Vec<Display>) {
        *self.lock() = displays;
    }

    fn lock(&self) -> MutexGuard<'_, Vec<Display>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Displays for MockDisplays {
    fn displays(&self) -> Result<Vec<Display>> {
        Ok(self.lock().clone())
    }
}
