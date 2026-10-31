//! 测试用的假平台：可注入输入事件，并记录注入结果。
//!
//! 供上层 crate 的场景测试使用。层级：L2。

#![forbid(unsafe_code)]

pub mod autostart;
pub mod capture;
pub mod clipboard;
pub mod display;
pub mod inject;
pub mod keymap;
pub mod permission;
