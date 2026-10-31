//! 网络层：TCP + rustls、输入连接与数据连接、心跳、重连、mDNS 发现。
//!
//! 目前提供：在任意字节流上收发协议帧（[`Framed`]）、握手流程、TCP 连接的建立。
//! TLS 将以包装字节流的方式加入，上层代码不受影响。
//!
//! 层级：L2。

#![forbid(unsafe_code)]

mod error;
mod framed;
mod handshake;
mod tcp;

pub use error::{Error, Result};
pub use framed::Framed;
pub use handshake::{accept, initiate};
pub use tcp::{connect, wrap};
