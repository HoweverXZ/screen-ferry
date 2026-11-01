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
    #[error("同一会话的几条连接连到了不同的设备")]
    PeerMismatch,
    #[error("局域网广播的指纹与对方的证书不符，可能有设备在冒充，已跳过")]
    AdvertisedMismatch,
    #[error(
        "局域网里有多台未配对的服务端：{0}。请在设置界面里按地址连接，或者用 connect <地址> 指定一台"
    )]
    AmbiguousServers(String),
    #[error("服务端地址无效：{0}")]
    BadAddress(String),
    #[error(transparent)]
    Ipc(#[from] ferry_ipc::Error),
    #[error("还没有连上对方，或者对方不支持传文件")]
    NotConnected,
    #[error("{0}")]
    Transfer(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
