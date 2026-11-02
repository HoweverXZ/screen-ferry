//! 更新清单的错误。

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("这份程序里没有发布公钥，无法校验更新")]
    NoKeys,
    #[error("发布公钥格式不对：{0}")]
    BadKey(String),
    #[error("签名格式不对：{0}")]
    BadSignature(String),
    #[error("签名和内置的发布公钥对不上")]
    Untrusted,
    #[error("更新清单格式不对：{0}")]
    BadManifest(String),
    #[error("不认识的更新清单格式 {0}")]
    UnknownSchema(u32),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
