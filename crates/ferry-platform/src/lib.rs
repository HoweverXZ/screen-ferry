//! 平台抽象：输入捕获与注入、按键映射、剪贴板、显示器、权限、开机自启、主线程事件循环的 trait 定义。
//!
//! 只有接口，没有实现。每个模块与 `ferry-platform-*` 中的同名文件一一对应。
//!
//! 层级：L1。

#![forbid(unsafe_code)]

mod autostart;
mod capture;
mod clipboard;
mod display;
mod error;
mod event_loop;
mod inject;
mod keymap;
mod permission;

pub use autostart::Autostart;
pub use capture::{CaptureEvent, CaptureSink, InputCapture};
pub use clipboard::Clipboard;
pub use display::Displays;
pub use error::{Error, Result};
pub use event_loop::{Dispatcher, EventLoop, MainTask};
pub use inject::InputInject;
pub use permission::Permissions;
