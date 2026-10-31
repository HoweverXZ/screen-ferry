//! 开机自启：`Autostart` 的测试实现，只记在内存里。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ferry_platform::{Autostart, Result};

/// 克隆后共用同一个开关。
#[derive(Clone, Debug, Default)]
pub struct MockAutostart(Arc<AtomicBool>);

impl MockAutostart {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Autostart for MockAutostart {
    fn is_enabled(&self) -> Result<bool> {
        Ok(self.0.load(Ordering::SeqCst))
    }

    fn set_enabled(&self, enabled: bool) -> Result<()> {
        self.0.store(enabled, Ordering::SeqCst);
        Ok(())
    }
}
