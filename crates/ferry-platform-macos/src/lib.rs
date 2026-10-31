//! macOS 平台实现，基于 objc2 系列 crate。
//!
//! 仅在 macOS 上编译。层级：L2。

#![cfg(target_os = "macos")]

mod autostart;
mod capture;
mod clipboard;
mod display;
mod inject;
mod keymap;
mod permission;

pub use capture::MacCapture;
pub use clipboard::MacClipboard;
pub use display::MacDisplays;
pub use keymap::{hid_code, media_key};
pub use permission::MacPermissions;
