//! Windows 平台实现，基于 windows-rs 与 Win32 API。
//!
//! 仅在 Windows 上编译。层级：L2。

#![cfg(target_os = "windows")]

pub mod autostart;
pub mod capture;
pub mod clipboard;
pub mod display;
pub mod inject;
pub mod keymap;
pub mod permission;
