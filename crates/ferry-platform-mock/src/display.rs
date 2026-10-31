//! 显示器信息：`Displays` 的测试实现。

use ferry_platform::{Displays, Result};
use ferry_types::Display;

/// 假的显示器列表。
#[derive(Clone, Debug, Default)]
pub struct MockDisplays(pub Vec<Display>);

impl Displays for MockDisplays {
    fn displays(&self) -> Result<Vec<Display>> {
        Ok(self.0.clone())
    }
}
