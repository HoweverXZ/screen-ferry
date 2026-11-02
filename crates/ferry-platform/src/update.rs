//! 自己更新：定义 `Updater` trait（ADR 0014）。
//!
//! 更新包由上层下载、核对过大小和 SHA-256 后才交到这里。安装分三步：`prepare` 做费时的准备，
//! `launch` 启动新版本，新版本起来后 `confirm`，没起来 `rollback`。
//!
//! - macOS：解压，核对新应用的签名满足本机这份的“指定要求”（同一个开发者、同一个 bundle id）
//!   并且经过公证，再一步换掉新旧应用，旧的留作备份；`launch` 打开新应用。
//! - Windows：`launch` 以管理员身份静默运行安装包（弹一次 UAC），安装包会请正在运行的后台退出、
//!   装好后重新启动它；`confirm`、`rollback` 什么也不做。

use std::path::{Path, PathBuf};

use crate::error::Result;

pub trait Updater: Send + Sync {
    /// 更新清单里这个平台的更新包叫什么，例如 `macos-universal`、`windows-x86_64`。不支持时为 `None`。
    fn platform(&self) -> Option<&'static str>;

    /// 这份程序能不能自己装更新。不能时（例如临时签名的开发版本、不是用安装包装的、
    /// 放在没有写权限的位置）返回原因，界面请用户去下载页面手动安装。
    fn can_install(&self) -> Result<()>;

    /// 系统设置里的 HTTPS 代理，形如 `http://127.0.0.1:7890`；没有设置时为 `None`。
    fn proxy(&self) -> Option<String>;

    /// 安装前的准备，可能要几秒，放在后台线程里调用。`version` 是清单里的版本号，用来核对。
    fn prepare(&self, package: &Path, version: &str) -> Result<Prepared>;

    /// 启动新版本。Windows 上用户在 UAC 里点了“否”时返回 [`Error::Cancelled`](crate::Error::Cancelled)。
    fn launch(&self, prepared: &Prepared) -> Result<()>;

    /// 新版本起来了：删掉备份。
    fn confirm(&self, prepared: &Prepared);

    /// 新版本没起来：换回旧版本。之后再 `launch` 启动的就是旧版本。
    fn rollback(&self, prepared: &Prepared) -> Result<()>;
}

/// [`Updater::prepare`] 的结果。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prepared {
    /// `launch` 启动什么：macOS 是换好的应用，Windows 是安装包。
    pub target: PathBuf,
    /// macOS：旧应用的备份；Windows 没有。
    pub backup: Option<PathBuf>,
    /// 为真时由当前进程退出、等新版本起来（macOS）；为假时由安装包请当前进程退出（Windows）。
    pub restart_self: bool,
}
