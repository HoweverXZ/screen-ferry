//! 本机进程通信：后台与设置界面之间的消息定义与传输。
//!
//! 后台监听 `127.0.0.1` 的随机端口，把端口和随机令牌写进 [`Endpoint`] 文件；
//! 界面读取它连接，第一条消息带上令牌。之后每行一条 JSON：界面发 [`Request`]，
//! 后台回 [`Notice`]。完整说明见 `docs/ipc.md` 与 ADR 0004。
//!
//! 层级：L2。

#![forbid(unsafe_code)]

mod connection;
mod endpoint;
mod error;
mod message;

pub use connection::{Connection, Incoming, Listener, Reader, Writer, connect};
pub use endpoint::Endpoint;
pub use error::{Error, Result};
pub use message::{
    ClipboardImage, ClipboardSync, DialError, FoundDevice, HourActivity, Link, LinkStatus, Local,
    LogEntry, LogIcon, LogKind, MAX_LINE, Notice, PairingPrompt, PeerInfo, PeerRef, Problem,
    ProblemKind, Request, Settings, SettingsPatch, State, VERSION,
};
