//! 存储错误。

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("无法确定数据目录：缺少环境变量 {0}")]
    NoDataDir(&'static str),
    #[error("读写 {path} 失败：{source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{path} 格式不对：{detail}")]
    Format { path: PathBuf, detail: String },
    #[error("{path} 由更新版本的 ScreenFerry 写入（schema_version = {found}），请升级")]
    TooNew { path: PathBuf, found: u32 },
    #[error(
        "设备身份文件损坏（{path}）：{source}。删除 identity 目录可重新生成，但需要与所有设备重新配对"
    )]
    Identity {
        path: PathBuf,
        source: ferry_trust::Error,
    },
    #[error(transparent)]
    Trust(#[from] ferry_trust::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
