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
    #[error(transparent)]
    Store(#[from] ferry_store::Error),
    #[error(transparent)]
    Trust(#[from] ferry_trust::Error),
    #[error("配对被拒绝")]
    PairingRejected,
    #[error("配对失败：{0}")]
    PairingFailed(&'static str),
    #[error("数据连接的对方与输入连接不是同一台设备")]
    PeerMismatch,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
