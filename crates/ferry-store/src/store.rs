//! 数据目录的读写入口。

use std::fs;
use std::path::{Path, PathBuf};

use ferry_trust::{Identity, PeerList};
use ferry_types::{Config, ModifierPreset};
use ring::digest::{SHA256, digest};

use crate::config;
use crate::dirs::{default_cache_dir, default_data_dir, default_downloads_dir};
use crate::error::{Error, Result};
use crate::file::{self, io};
use crate::peers;

/// 一个数据目录（及其对应的缓存目录）。只读写其中的文件，不缓存内容，多次打开同一目录也不会不一致。
#[derive(Clone, Debug)]
pub struct Store {
    root: PathBuf,
    /// 运行时文件（例如 `ipc.json`）所在的目录。
    cache: PathBuf,
    /// 收到的文件放在这里。
    downloads: PathBuf,
}

impl Store {
    /// 打开本机默认的数据目录，不存在时创建。
    pub fn open_default() -> Result<Self> {
        let mut store = Self::open(default_data_dir()?)?;
        store.cache = default_cache_dir()?;
        store.downloads = default_downloads_dir()?;
        Ok(store)
    }

    /// 打开指定的数据目录，不存在时创建。运行时文件和收到的文件（`received/`）也放在这个目录里，
    /// 这样用 `--data-dir` 运行的几份后台互不干扰。
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        fs::create_dir_all(&root).map_err(|e| io(&root, e))?;
        Ok(Self {
            cache: root.clone(),
            downloads: root.join("received"),
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

    /// 用户设置。文件不存在时为缺省设置（不会自动写入）。旧版本的文件读进来时迁移到当前版本，
    /// 下次保存时才写回。
    pub fn config(&self) -> Result<Config> {
        let path = self.config_path();
        let Some(data) = file::read(&path)? else {
            return Ok(Config::default());
        };
        let parsed = config::parse(&path, &data)?;
        let mut config = parsed.config;
        // 版本 3 关掉了“Command 当作 Ctrl”：每台已配对的设备都改为原样发送。
        // 读不到已配对设备时只是少迁移这一项，不影响用。
        if parsed.legacy_swap_cmd_ctrl == Some(false) {
            let peers = self.peers().unwrap_or_default();
            for peer in peers.iter() {
                let mut device = config.device(&peer.fingerprint.to_hex());
                device.modifiers = ModifierPreset::Plain;
                config.set_device(device);
            }
        }
        Ok(config)
    }

    pub fn save_config(&self, config: &Config) -> Result<()> {
        file::write(&self.config_path(), config::render(config).as_bytes())
    }

    /// 后台写给设置界面的连接信息（端口与令牌），见 ADR 0004。
    pub fn ipc_endpoint_path(&self) -> PathBuf {
        self.cache.join("ipc.json")
    }

    /// 日志目录（缓存目录下的 `logs/`），可以随时清空。
    pub fn logs_dir(&self) -> PathBuf {
        self.cache.join("logs")
    }

    /// 正在接收的文件（缓存目录下的 `partial/`），每次传输一个子目录。可以随时清空。
    pub fn partial_dir(&self) -> PathBuf {
        self.cache.join("partial")
    }

    /// 收完的文件放在哪里：本机的“下载”文件夹；用 `--data-dir` 打开时为其中的 `received/`。
    pub fn downloads_dir(&self) -> PathBuf {
        self.downloads.clone()
    }

    /// 按内容存一份图片或文件，返回它的路径：数据目录下的 `blobs/ab/cdef…`，名字是内容的 SHA-256。
    /// 同样的内容只存一份。
    pub fn put_blob(&self, data: &[u8]) -> Result<PathBuf> {
        let hash = digest(&SHA256, data);
        let hex: String = hash.as_ref().iter().map(|b| format!("{b:02x}")).collect();
        let path = self.blobs_dir().join(&hex[..2]).join(&hex[2..]);
        if !path.exists() {
            file::write(&path, data)?;
        }
        Ok(path)
    }

    /// 删除 [`put_blob`](Self::put_blob) 存的一份。已经不在时什么也不做；`blobs/` 以外的路径不碰。
    pub fn remove_blob(&self, path: &Path) -> Result<()> {
        if !path.starts_with(self.blobs_dir()) {
            return Ok(());
        }
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(io(path, e)),
        }
    }

    /// 清空 `blobs/`。剪贴板历史做好之前，这里只放最近一次剪贴板图片的预览，启动时清掉上次留下的。
    pub fn clear_blobs(&self) -> Result<()> {
        let dir = self.blobs_dir();
        match fs::remove_dir_all(&dir) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(io(&dir, e)),
        }
    }

    fn blobs_dir(&self) -> PathBuf {
        self.root.join("blobs")
    }

    fn config_path(&self) -> PathBuf {
        self.root.join("config.toml")
    }

    fn peers_path(&self) -> PathBuf {
        self.root.join("peers.toml")
    }
}
