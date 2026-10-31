//! 存储：配置文件、SQLite、运行时数据目录。
//!
//! 只有本 crate 知道数据目录的位置。目前保存用户设置（`config.toml`）、设备身份（`identity/`）
//! 和已信任设备（`peers.toml`），并指出运行时文件 `ipc.json` 的位置。层级：L2。

#![forbid(unsafe_code)]

mod config;
mod dirs;
mod error;
mod file;
mod peers;
mod store;

pub use dirs::{default_cache_dir, default_data_dir};
pub use error::{Error, Result};
pub use store::Store;
