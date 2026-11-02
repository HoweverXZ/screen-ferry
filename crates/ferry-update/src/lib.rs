//! 版本更新的纯逻辑：更新清单的格式、签名校验、判断有没有新版本（ADR 0014）。
//!
//! 发布时 CI 写一份 `update.json`（版本、发布和过期时间、各平台更新包的地址、大小和 SHA-256），
//! 用发布私钥做 minisign 签名，和安装包一起放在 GitHub Releases 上。程序取回清单后：
//!
//! 1. 用内置的公钥（[`RELEASE_KEYS`]）校验签名，任何一把对得上就行；
//! 2. 发布时间早于见过的那份就不认（防回滚），过了过期时间也不认（防冻结）；
//! 3. 版本比本机新才算有更新，下载的文件大小和 SHA-256 必须和清单一致。
//!
//! 不做网络和文件读写：取清单、下载在 `ferry-net`，安装在 `ferry-platform-*`。层级：L1。

#![forbid(unsafe_code)]

mod error;
mod keys;
mod manifest;
mod verdict;

pub use error::{Error, Result};
pub use keys::RELEASE_KEYS;
pub use manifest::{Asset, Manifest, Notes, Release, SCHEMA};
pub use semver::Version;
pub use verdict::{Verdict, judge};
