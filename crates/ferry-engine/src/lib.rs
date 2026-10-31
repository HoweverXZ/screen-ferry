//! 运行时组装：把 core、platform、net、store、ipc 组合成可运行的后台引擎。
//!
//! 只依赖 ferry-platform 的 trait，具体平台实现由应用注入。层级：L3。
//!
//! 目前是端到端验证原型：一台服务端（拥有键鼠）对一台客户端，只转发鼠标。

#![forbid(unsafe_code)]

mod client;
mod error;
mod heartbeat;
mod link;
mod server;

pub use client::{ClientConfig, connect};
pub use error::{Error, Result};
pub use link::DEFAULT_PORT;
pub use server::{ServerConfig, serve};
