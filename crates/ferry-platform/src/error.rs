//! 平台错误。

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("缺少系统权限：{0}")]
    PermissionDenied(&'static str),
    #[error("系统调用 {call} 失败：{detail}")]
    Os { call: &'static str, detail: String },
    #[error("此平台暂不支持：{0}")]
    Unsupported(&'static str),
    #[error("用户取消了")]
    Cancelled,
    #[error("{0}")]
    Failed(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
