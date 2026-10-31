//! `peers.toml`：已信任的设备。
//!
//! ```toml
//! schema_version = 1
//!
//! [[peer]]
//! name = "办公室 PC"
//! fingerprint = "3f9a…（64 位十六进制）"
//! ```

use std::path::Path;

use ferry_trust::{Peer, PeerList};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

const SCHEMA_VERSION: u32 = 1;

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
            Ok(Peer {
                name: entry.name,
                fingerprint,
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
            })
            .collect(),
    };
    toml::to_string(&file).expect("字符串和整数总能写成 TOML")
}
