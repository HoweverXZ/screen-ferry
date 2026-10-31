//! macOS 平台实现，基于 objc2 系列 crate。
//!
//! 仅在 macOS 上编译。层级：L2。

#![cfg(target_os = "macos")]

pub mod autostart;
pub mod capture;
pub mod clipboard;
pub mod display;
pub mod inject;
pub mod keymap;
pub mod permission;
