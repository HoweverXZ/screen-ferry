//! 系统权限：定义 `Permissions` trait。

use crate::error::Result;

/// 检查系统权限。
pub trait Permissions {
    /// 检查捕获和注入键鼠所需的权限。缺少时返回 [`Error::PermissionDenied`](crate::Error)，
    /// 其中说明要去哪里授权。
    fn check_input(&self) -> Result<()>;
}
