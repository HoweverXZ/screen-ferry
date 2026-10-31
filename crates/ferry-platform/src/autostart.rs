//! 开机自启：定义 `Autostart` trait。

use crate::error::Result;

/// 登录时自动启动后台。是否开启以系统中的登记为准，不另外记录。
pub trait Autostart {
    /// 是否已登记为登录时启动。
    fn is_enabled(&self) -> Result<bool>;

    /// 登记或取消登录时启动。
    fn set_enabled(&self, enabled: bool) -> Result<()>;
}
