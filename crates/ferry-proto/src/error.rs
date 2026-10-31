//! 编解码错误。
//!
//! 任何解码错误都表示对方违反协议，调用方应发送 `Bye` 并关闭连接。

use crate::handshake::Channel;

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    #[error("未知消息类型 {0:#06x}")]
    UnknownMessage(u16),
    #[error("消息类型 {ty:#06x} 不能出现在{channel}连接上")]
    WrongChannel { ty: u16, channel: Channel },
    #[error("帧过大：{len} 字节，上限 {max} 字节")]
    FrameTooLarge { len: usize, max: usize },
    #[error("消息体不完整")]
    Truncated,
    #[error("消息体末尾有多余字节")]
    TrailingBytes,
    #[error("非法取值：{0}")]
    InvalidValue(&'static str),
    #[error("超出上限：{0}")]
    LimitExceeded(&'static str),
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum EncodeError {
    #[error("这条消息不能在{0}连接上发送")]
    WrongChannel(Channel),
    #[error("帧过大：{len} 字节，上限 {max} 字节")]
    FrameTooLarge { len: usize, max: usize },
    #[error("超出上限：{0}")]
    LimitExceeded(&'static str),
}
