//! 全项目共用的领域类型。
//!
//! 设备与屏幕 ID、HID 键码、输入事件、几何、配置结构。
//!
//! 层级：L0。不依赖任何其他 ferry crate。

#![forbid(unsafe_code)]

mod clipboard;
mod display;
mod geometry;
mod input;
mod key;
mod pointer;

pub use clipboard::ClipboardFormat;
pub use display::{Display, DisplayId};
pub use geometry::{Point, Rect};
pub use input::InputEvent;
pub use key::{KeyAction, KeyCode, LockKeys};
pub use pointer::{MouseButton, WHEEL_NOTCH};
