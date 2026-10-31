//! 引擎错误。

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Net(#[from] ferry_net::Error),
    #[error(transparent)]
    Platform(#[from] ferry_platform::Error),
    #[error("网络错误：{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}超时")]
    Timeout(&'static str),
    #[error("心跳超时：对方长时间没有响应")]
    HeartbeatTimeout,
    #[error("{0}没有可用的显示器")]
    NoDisplays(&'static str),
    #[error("对方违反协议：{0}")]
    Protocol(&'static str),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
