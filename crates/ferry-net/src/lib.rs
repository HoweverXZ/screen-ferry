//! 网络层：TCP + rustls、输入连接与数据连接、心跳、重连、mDNS 发现。
//!
//! 目前提供：在任意字节流上收发协议帧（[`Framed`]）、握手流程、
//! TLS 1.3 加密的 TCP 连接（[`Tls`]），握手后给出对方证书的指纹；
//! 局域网 mDNS 广播与发现（[`discovery`]）；本机的局域网地址（[`local_addresses`]）；
//! 版本更新用的 HTTPS 下载（[`Https`]）。
//!
//! 层级：L2。

#![forbid(unsafe_code)]

pub mod discovery;

mod addresses;
mod error;
mod framed;
mod handshake;
mod https;
mod tls;

pub use addresses::local_addresses;
pub use error::{Error, Result};
pub use framed::Framed;
pub use handshake::{accept, initiate};
pub use https::{Downloaded, Https};
pub use tls::{ClientStream, ServerStream, Tls};
