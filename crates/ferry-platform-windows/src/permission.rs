//! 系统权限：`Permissions` 的 Windows 实现。
//!
//! 普通键鼠注入不需要额外授权。向管理员权限的窗口注入会被系统拦截（UIPI），
//! 这需要以管理员身份运行或使用 uiAccess，留到 Windows 服务阶段处理。

use ferry_platform::{Permissions, Result};

/// 本机权限检查。
#[derive(Clone, Copy, Debug, Default)]
pub struct WinPermissions;

impl Permissions for WinPermissions {
    fn check_input(&self) -> Result<()> {
        Ok(())
    }
}
