//! 全项目共用的领域类型。
//!
//! 设备与屏幕 ID、HID 键码、输入事件、几何、配置结构、文件传输编号。
//!
//! 层级：L0。不依赖任何其他 ferry crate。

#![forbid(unsafe_code)]

mod clipboard;
mod config;
mod display;
mod geometry;
mod input;
mod key;
mod pointer;
mod transfer;

pub use clipboard::{ClipboardContent, ClipboardFormat};
pub use config::{Config, Role, Theme};
pub use display::{Display, DisplayId};
pub use geometry::{Point, Rect, Side};
pub use input::InputEvent;
pub use key::{KeyAction, KeyCode, LockKeys};
pub use pointer::{MouseButton, WHEEL_NOTCH};
pub use transfer::{ParseTransferIdError, TransferId};
