//! 运行时组装：把 core、platform、net、store、ipc 组合成可运行的后台引擎。
//!
//! 只依赖 ferry-platform 的 trait，具体平台实现由应用注入。层级：L3。
//!
//! 目前是端到端验证原型：一台服务端（拥有键鼠）对一台客户端，共享键鼠和剪贴板文本。
//! 连接全程 TLS 1.3 加密，首次连接需要两边的用户核对配对码。

#![forbid(unsafe_code)]

mod client;
mod clipboard;
mod error;
mod heartbeat;
mod link;
mod pairing;
mod server;

pub use client::{ClientConfig, ServerAddr, connect};
pub use error::{Error, Result};
pub use link::DEFAULT_PORT;
pub use pairing::{ConfirmPairing, PairingRequest};
pub use server::{ServerConfig, serve};
