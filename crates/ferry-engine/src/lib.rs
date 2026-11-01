//! 运行时组装：把 core、platform、net、store、ipc 组合成可运行的后台引擎。
//!
//! 只依赖 ferry-platform 的 trait，具体平台实现由应用注入。层级：L3。
//!
//! 一台服务端（拥有键鼠）对几台客户端，共享键鼠、剪贴板（文本、带格式的文本、图片），并能互传文件（见 [`Files`]）。连接全程 TLS 1.3 加密，
//! 首次连接需要两边的用户核对配对码。[`resident`] 在此之上提供常驻运行：读配置、
//! 启停引擎，并经本机通信与设置界面交换状态和命令。

#![forbid(unsafe_code)]

mod client;
mod clipboard;
mod edge_drop;
mod error;
mod events;
mod files;
mod heartbeat;
mod link;
mod pairing;
pub mod resident;
mod server;

pub use client::{ClientConfig, ServerAddr, connect};
pub use clipboard::{ClipLimit, DEFAULT_CLIP_LIMIT};
pub use edge_drop::{Edge, EdgeDrop, EdgeFeed, Pointer};
pub use error::{Error, Result};
pub use events::{
    ClipImage, ClipSummary, Event, Events, FoundServer, TransferState, TransferUpdate,
};
pub use files::Files;
pub use link::DEFAULT_PORT;
pub use pairing::{ConfirmPairing, PairingRequest};
pub use server::{Live, ServerConfig, serve};
