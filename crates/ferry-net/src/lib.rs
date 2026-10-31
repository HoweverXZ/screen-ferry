//! 网络层：TCP + rustls、输入连接与数据连接、心跳、重连、mDNS 发现。
//!
//! 目前提供：在任意字节流上收发协议帧（[`Framed`]）、握手流程、
//! TLS 1.3 加密的 TCP 连接（[`Tls`]），握手后给出对方证书的指纹；
//! 局域网 mDNS 广播与发现（[`discovery`]）。
//!
//! 层级：L2。

#![forbid(unsafe_code)]

pub mod discovery;

mod error;
mod framed;
mod handshake;
mod tls;

pub use error::{Error, Result};
pub use framed::Framed;
pub use handshake::{accept, initiate};
pub use tls::{ClientStream, ServerStream, Tls};
