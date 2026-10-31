//! `ipc.json`：后台写给界面的连接信息。文件位置由 `ferry-store` 决定。

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// 后台在哪个端口、用哪个令牌。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Endpoint {
    pub port: u16,
    /// 64 位十六进制的随机令牌。
    pub token: String,
    /// 后台的进程号，仅供排查问题。
    pub pid: u32,
    /// 协议版本，见 [`crate::VERSION`]。
    pub version: u32,
}

impl Endpoint {
    /// 读取连接信息；文件不存在时为 `None`。
    pub fn load(path: &Path) -> std::io::Result<Option<Self>> {
        match fs::read(path) {
            Ok(data) => serde_json::from_slice(&data)
                .map(Some)
                .map_err(|e| std::io::Error::new(ErrorKind::InvalidData, e)),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// 写入连接信息：先写临时文件再改名，界面不会读到半个文件。
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let mut tmp = path.as_os_str().to_owned();
        tmp.push(".tmp");
        let data = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        fs::write(&tmp, data)?;
        fs::rename(&tmp, path)
    }

    /// 后台退出时删除连接信息。只删属于 `pid` 的文件，不误删新启动的那份后台写下的。
    pub fn remove(path: &Path, pid: u32) {
        if matches!(Self::load(path), Ok(Some(e)) if e.pid == pid) {
            let _ = fs::remove_file(path);
        }
    }
}
