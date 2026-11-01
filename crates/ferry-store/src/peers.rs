//! `peers.toml`：已信任的设备。
//!
//! ```toml
//! schema_version = 2
//!
//! [[peer]]
//! name = "办公室 PC"
//! fingerprint = "3f9a…（64 位十六进制）"
//! addr = "192.168.2.18:24810"   # 上次连上时的地址，可以没有
//! ```
//!
//! 版本 1 → 2：新增可选的 `addr`，版本 1 的文件读进来就是没有地址的版本 2。

use std::path::Path;

use ferry_trust::{Peer, PeerList};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

const SCHEMA_VERSION: u32 = 2;

#[derive(Serialize, Deserialize)]
struct PeersFile {
    schema_version: u32,
    #[serde(default, rename = "peer")]
    peers: Vec<Entry>,
}

#[derive(Serialize, Deserialize)]
struct Entry {
    name: String,
    fingerprint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    addr: Option<String>,
}

pub(crate) fn parse(path: &Path, data: &[u8]) -> Result<PeerList> {
    let format = |detail: String| Error::Format {
        path: path.to_owned(),
        detail,
    };
    let text = std::str::from_utf8(data).map_err(|_| format("不是 UTF-8 文本".into()))?;
    let file: PeersFile = toml::from_str(text).map_err(|e| format(e.to_string()))?;
    if file.schema_version > SCHEMA_VERSION {
        return Err(Error::TooNew {
            path: path.to_owned(),
            found: file.schema_version,
        });
    }
    file.peers
        .into_iter()
        .map(|entry| {
            let fingerprint = entry
                .fingerprint
                .parse()
                .map_err(|e: ferry_trust::Error| format(format!("{}：{e}", entry.name)))?;
            // 地址写坏了只是少一个兜底，不影响信任关系。
            let addr = entry.addr.and_then(|a| a.parse().ok());
            Ok(Peer {
                name: entry.name,
                fingerprint,
                addr,
            })
        })
        .collect()
}

pub(crate) fn render(peers: &PeerList) -> String {
    let file = PeersFile {
        schema_version: SCHEMA_VERSION,
        peers: peers
            .iter()
            .map(|p| Entry {
                name: p.name.clone(),
                fingerprint: p.fingerprint.to_hex(),
                addr: p.addr.map(|a| a.to_string()),
            })
            .collect(),
    };
    toml::to_string(&file).expect("字符串和整数总能写成 TOML")
}
