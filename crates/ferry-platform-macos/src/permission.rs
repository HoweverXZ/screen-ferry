//! 系统权限：`Permissions` 的 macOS 实现。
//!
//! 拦截和注入键鼠需要"辅助功能"权限。从终端启动时，权限授予的是终端程序。

use ferry_platform::{Error, Permissions, Result};

pub(crate) const ACCESSIBILITY: &str = "需要“辅助功能”权限：请在「系统设置 → 隐私与安全性 → 辅助功能」中允许运行 screenferry 的程序（例如“终端”），然后重新启动";

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    /// 当前进程是否已获得辅助功能权限。返回 C 的 `Boolean`。
    safe fn AXIsProcessTrusted() -> u8;
}

/// 本机权限检查。
#[derive(Clone, Copy, Debug, Default)]
pub struct MacPermissions;

impl Permissions for MacPermissions {
    fn check_input(&self) -> Result<()> {
        if AXIsProcessTrusted() != 0 {
            Ok(())
        } else {
            Err(Error::PermissionDenied(ACCESSIBILITY))
        }
    }
}
