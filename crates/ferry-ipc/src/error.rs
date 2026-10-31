//! 本机通信的错误。

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("本机通信失败：{0}")]
    Io(#[from] std::io::Error),
    #[error("消息格式不对：{0}")]
    Json(#[from] serde_json::Error),
    #[error("单条消息超过 {} 字节", crate::MAX_LINE)]
    LineTooLong,
    #[error("令牌不对")]
    BadToken,
    #[error("协议版本不一致：本机 {ours}，对方 {theirs}")]
    Version { ours: u32, theirs: u32 },
    #[error("握手超时")]
    Timeout,
    #[error("连接已关闭")]
    Closed,
    #[error("握手时收到了意料之外的消息")]
    Unexpected,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
