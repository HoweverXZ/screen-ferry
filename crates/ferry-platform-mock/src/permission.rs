//! 系统权限：`Permissions` 的测试实现。

use ferry_platform::{Error, Permissions, Result};

/// 假的权限检查。
#[derive(Clone, Copy, Debug)]
pub struct MockPermissions {
    pub granted: bool,
}

impl Permissions for MockPermissions {
    fn check_input(&self) -> Result<()> {
        if self.granted {
            Ok(())
        } else {
            Err(Error::PermissionDenied("测试中模拟的缺少权限"))
        }
    }
}
