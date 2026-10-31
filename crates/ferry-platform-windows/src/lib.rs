//! Windows 平台实现，基于 windows-rs 与 Win32 API。
//!
//! 进程会被设为“每显示器 DPI 感知 V2”，所有坐标都是物理像素。
//!
//! 仅在 Windows 上编译。层级：L2。

#![cfg(target_os = "windows")]

mod autostart;
mod capture;
mod clipboard;
mod display;
mod inject;
mod keymap;
mod permission;

pub use display::WinDisplays;
pub use inject::WinInject;
pub use permission::WinPermissions;
