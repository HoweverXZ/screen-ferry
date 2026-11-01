//! 核心逻辑：屏幕布局、边缘切换、多台设备的布局与切换、会话状态机、按键状态跟踪、收到的文件名规则、边缘投递胶囊。
//!
//! 层级：L1。纯逻辑，无 IO、无异步。

#![forbid(unsafe_code)]

pub mod capsule;
mod edge_switch;
pub mod file_name;
mod key_remap;
mod layout;
mod neighbors;
mod pressed;

pub use edge_switch::{EdgeSwitch, Transition};
pub use key_remap::KeyRemap;
pub use layout::{Side, bounds};
pub use neighbors::{Neighbors, Place};
pub use pressed::Pressed;
