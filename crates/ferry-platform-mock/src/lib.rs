//! 测试用的假平台：可注入输入事件，并记录注入结果。
//!
//! 每个类型都可以克隆，克隆体共享状态：一份交给被测代码，一份留在测试里观察。
//!
//! 供上层 crate 的场景测试使用。层级：L2。

#![forbid(unsafe_code)]

mod autostart;
mod capture;
mod clipboard;
mod display;
mod event_loop;
mod inject;
mod keymap;
mod permission;

pub use autostart::MockAutostart;
pub use capture::MockCapture;
pub use clipboard::MockClipboard;
pub use display::MockDisplays;
pub use event_loop::MockEventLoop;
pub use inject::MockInject;
pub use permission::MockPermissions;
