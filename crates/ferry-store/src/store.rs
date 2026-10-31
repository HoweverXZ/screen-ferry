//! 数据目录的读写入口。

use std::fs;
use std::path::{Path, PathBuf};

use ferry_trust::{Identity, PeerList};
use ferry_types::Config;

use crate::config;
use crate::dirs::{default_cache_dir, default_data_dir};
use crate::error::{Error, Result};
use crate::file::{self, io};
use crate::peers;

/// 一个数据目录（及其对应的缓存目录）。只读写其中的文件，不缓存内容，多次打开同一目录也不会不一致。
#[derive(Clone, Debug)]
pub struct Store {
    root: PathBuf,
    /// 运行时文件（例如 `ipc.json`）所在的目录。
    cache: PathBuf,
}

impl Store {
    /// 打开本机默认的数据目录，不存在时创建。
    pub fn open_default() -> Result<Self> {
        let mut store = Self::open(default_data_dir()?)?;
        store.cache = default_cache_dir()?;
        Ok(store)
    }

    /// 打开指定的数据目录，不存在时创建。运行时文件也放在这个目录里，
    /// 这样用 `--data-dir` 运行的几份后台互不干扰。
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        fs::create_dir_all(&root).map_err(|e| io(&root, e))?;
        Ok(Self {
            cache: root.clone(),
            root,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 读取本机身份；还没有时生成一个并保存。`name` 只在生成时写进证书。
    pub fn identity(&self, name: &str) -> Result<Identity> {
        let cert_path = self.root.join("identity").join("device.crt");
        let key_path = self.root.join("identity").join("device.key");
        match (file::read(&cert_path)?, file::read(&key_path)?) {
            (Some(cert), Some(key)) => {
                Identity::from_der(cert, key).map_err(|source| Error::Identity {
                    path: cert_path,
                    source,
                })
            }
            (None, None) => {
                let identity = Identity::generate(name)?;
                // 先写私钥再写证书：证书在，私钥就一定在。
                file::write(&key_path, identity.key_der())?;
                file::write(&cert_path, identity.cert_der())?;
                Ok(identity)
            }
            (cert, _) => Err(Error::Identity {
                path: if cert.is_some() { key_path } else { cert_path },
                source: ferry_trust::Error::InvalidIdentity("证书和私钥缺了一个"),
            }),
        }
    }

    /// 已信任的设备。文件不存在时为空列表。
    pub fn peers(&self) -> Result<PeerList> {
        let path = self.peers_path();
        match file::read(&path)? {
            Some(data) => peers::parse(&path, &data),
            None => Ok(PeerList::new()),
        }
    }

    pub fn save_peers(&self, list: &PeerList) -> Result<()> {
        file::write(&self.peers_path(), peers::render(list).as_bytes())
    }

    /// 用户设置。文件不存在时为缺省设置（不会自动写入）。
    pub fn config(&self) -> Result<Config> {
        let path = self.config_path();
        match file::read(&path)? {
            Some(data) => config::parse(&path, &data),
            None => Ok(Config::default()),
        }
    }

    pub fn save_config(&self, config: &Config) -> Result<()> {
        file::write(&self.config_path(), config::render(config).as_bytes())
    }

    /// 后台写给设置界面的连接信息（端口与令牌），见 ADR 0004。
    pub fn ipc_endpoint_path(&self) -> PathBuf {
        self.cache.join("ipc.json")
    }

    fn config_path(&self) -> PathBuf {
        self.root.join("config.toml")
    }

    fn peers_path(&self) -> PathBuf {
        self.root.join("peers.toml")
    }
}
