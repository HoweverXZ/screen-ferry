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
mod event_loop;
mod inject;
mod keymap;
mod overlay;
mod permission;

pub use autostart::WinAutostart;
pub use clipboard::{WinClipboard, text_from_windows, text_to_windows};
pub use display::WinDisplays;
pub use event_loop::WinEventLoop;
pub use inject::WinInject;
pub use keymap::{WinKey, win_key};
pub use overlay::WinOverlay;
pub use permission::WinPermissions;
