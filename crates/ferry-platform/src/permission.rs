//! 系统权限：定义 `Permissions` trait。

use crate::error::Result;

/// 检查系统权限。
pub trait Permissions {
    /// 检查捕获和注入键鼠所需的权限。缺少时返回 [`Error::PermissionDenied`](crate::Error)，
    /// 其中说明要去哪里授权。
    fn check_input(&self) -> Result<()>;

    /// 向系统申请这些权限：系统可能弹出授权提示，并把本程序加入授权列表，用户只需打开开关。
    /// 不需要申请的系统上与 [`check_input`](Self::check_input) 相同。
    fn request_input(&self) -> Result<()> {
        self.check_input()
    }
}
