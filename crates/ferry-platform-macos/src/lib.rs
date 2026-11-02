//! macOS 平台实现，基于 objc2 系列 crate。
//!
//! 仅在 macOS 上编译。层级：L2。

#![cfg(target_os = "macos")]

mod autostart;
mod capture;
mod clipboard;
mod display;
mod event_loop;
mod inject;
mod keymap;
mod overlay;
mod permission;
mod update;

pub use autostart::MacAutostart;
pub use capture::{MacCapture, natural_scrolling};
pub use clipboard::MacClipboard;
pub use display::MacDisplays;
pub use event_loop::MacEventLoop;
pub use inject::MacInject;
pub use keymap::{hid_code, mac_keycode, media_key, nx_key};
pub use overlay::{MacOverlay, render_png};
pub use permission::MacPermissions;
pub use update::{MacUpdater, parse_scutil_proxy};
