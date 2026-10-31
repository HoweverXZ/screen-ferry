//! 核心逻辑：屏幕布局、边缘切换、会话状态机、按键状态跟踪。
//!
//! 层级：L1。纯逻辑，无 IO、无异步。

#![forbid(unsafe_code)]

mod edge_switch;
mod key_remap;
mod layout;
mod pressed;

pub use edge_switch::{EdgeSwitch, Transition};
pub use key_remap::KeyRemap;
pub use layout::{Side, bounds};
pub use pressed::Pressed;
