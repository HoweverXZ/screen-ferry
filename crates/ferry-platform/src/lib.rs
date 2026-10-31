//! 平台抽象：输入捕获与注入、按键映射、剪贴板、显示器、权限、开机自启的 trait 定义。
//!
//! 只有接口，没有实现。每个模块与 `ferry-platform-*` 中的同名文件一一对应。
//!
//! 层级：L1。

#![forbid(unsafe_code)]

pub mod autostart;
pub mod capture;
pub mod clipboard;
pub mod display;
pub mod inject;
pub mod keymap;
pub mod permission;
