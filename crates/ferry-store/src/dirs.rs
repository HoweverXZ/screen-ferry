//! 运行时数据目录与缓存目录的位置，见 AGENTS.md 第 10 节。

use std::env;
use std::path::PathBuf;

use crate::error::{Error, Result};

const APP_DIR: &str = "ScreenFerry";

/// 本机默认的数据目录：
///
/// - macOS：`~/Library/Application Support/ScreenFerry/`
/// - Windows：`%APPDATA%\ScreenFerry\`
///
/// 这两处都只有当前用户能访问，身份私钥直接放在其中。
pub fn default_data_dir() -> Result<PathBuf> {
    // 按运行时的系统选择；平台差异只有这一处，不值得为它引入条件编译。
    match env::consts::OS {
        "windows" => Ok(var_dir("APPDATA")?.join(APP_DIR)),
        "macos" => Ok(var_dir("HOME")?
            .join("Library")
            .join("Application Support")
            .join(APP_DIR)),
        _ => match var_dir("XDG_DATA_HOME") {
            Ok(dir) => Ok(dir.join("screenferry")),
            Err(_) => Ok(var_dir("HOME")?.join(".local/share/screenferry")),
        },
    }
}

/// 本机默认的缓存目录，可以随时清空：
///
/// - macOS：`~/Library/Caches/ScreenFerry/`
/// - Windows：`%LOCALAPPDATA%\ScreenFerry\`
///
/// 这两处同样只有当前用户能访问。
pub fn default_cache_dir() -> Result<PathBuf> {
    match env::consts::OS {
        "windows" => Ok(var_dir("LOCALAPPDATA")?.join(APP_DIR)),
        "macos" => Ok(var_dir("HOME")?
            .join("Library")
            .join("Caches")
            .join(APP_DIR)),
        _ => match var_dir("XDG_CACHE_HOME") {
            Ok(dir) => Ok(dir.join("screenferry")),
            Err(_) => Ok(var_dir("HOME")?.join(".cache/screenferry")),
        },
    }
}

/// 本机的“下载”文件夹，收到的文件放在这里：
///
/// - macOS：`~/Downloads/`
/// - Windows：`%USERPROFILE%\Downloads\`
pub fn default_downloads_dir() -> Result<PathBuf> {
    match env::consts::OS {
        "windows" => Ok(var_dir("USERPROFILE")?.join("Downloads")),
        _ => Ok(var_dir("HOME")?.join("Downloads")),
    }
}

/// 当前用户的主目录（macOS 的 `HOME`，Windows 的 `USERPROFILE`）。导出诊断文件时换成 `~`，
/// 免得把用户名带出去。
pub fn home_dir() -> Option<PathBuf> {
    let name = if env::consts::OS == "windows" {
        "USERPROFILE"
    } else {
        "HOME"
    };
    var_dir(name).ok()
}

fn var_dir(name: &'static str) -> Result<PathBuf> {
    env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .ok_or(Error::NoDataDir(name))
}
