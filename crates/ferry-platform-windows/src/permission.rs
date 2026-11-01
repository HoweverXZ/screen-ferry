//! 系统权限：`Permissions` 的 Windows 实现。
//!
//! 普通键鼠注入不需要额外授权。管理员权限的窗口不接受本程序注入的输入。

use ferry_platform::{Permissions, Result};

/// 本机权限检查。
#[derive(Clone, Copy, Debug, Default)]
pub struct WinPermissions;

impl Permissions for WinPermissions {
    fn check_input(&self) -> Result<()> {
        Ok(())
    }
}
