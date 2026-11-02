//! HTTPS 下载，给版本更新用（ADR 0014）。阻塞调用，放在后台线程里。
//!
//! 证书用系统的校验（macOS 的钥匙串、Windows 的证书库），代理用系统设置里的（由平台层读出来
//! 交给这里），没有时按 `HTTPS_PROXY` 等环境变量。下载边写盘边算 SHA-256，不占内存；
//! 文件里已有一部分时接着下。

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use ring::digest::{Context, SHA256};
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};

use crate::error::{Error, Result};

/// 连接的超时，以及下载中途多久收不到数据就算断了。
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const READ_TIMEOUT: Duration = Duration::from_secs(30);

pub struct Https {
    agent: ureq::Agent,
}

/// 下载完的文件。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Downloaded {
    pub size: u64,
    /// 小写十六进制。
    pub sha256: String,
}

impl Https {
    /// `proxy` 形如 `http://127.0.0.1:7890`。`user_agent` 例如 `ScreenFerry/0.1.0`。
    pub fn new(proxy: Option<&str>, user_agent: &str) -> Result<Self> {
        let tls = TlsConfig::builder()
            .provider(TlsProvider::Rustls)
            .root_certs(RootCerts::PlatformVerifier)
            .unversioned_rustls_crypto_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .build();
        let mut config = ureq::Agent::config_builder()
            .tls_config(tls)
            .user_agent(user_agent)
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_per_read(Some(READ_TIMEOUT))
            .http_status_as_error(false);
        if let Some(proxy) = proxy {
            let proxy = ureq::Proxy::new(proxy).map_err(|e| Error::Http(e.to_string()))?;
            config = config.proxy(Some(proxy));
        }
        Ok(Self {
            agent: config.build().new_agent(),
        })
    }

    /// 取一个小文件（更新清单之类），超过 `limit` 字节就算错。
    pub fn fetch(&self, url: &str, limit: u64) -> Result<Vec<u8>> {
        let mut response = self.agent.get(url).call().map_err(http)?;
        check_status(response.status().as_u16())?;
        response
            .body_mut()
            .with_config()
            .limit(limit)
            .read_to_vec()
            .map_err(|e| match e {
                ureq::Error::BodyExceedsLimit(_) => Error::TooLarge,
                e => http(e),
            })
    }

    /// 下载到 `path`。文件里已有一部分时请服务器从那里接着发；服务器不支持就从头下。
    /// `total` 是预期的大小，超过就停。`progress(已下载, total)` 不时被调用；
    /// `cancel` 变为真时尽快停下，返回 [`Error::Cancelled`]，已下载的部分留着下次接着用。
    pub fn download(
        &self,
        url: &str,
        path: &Path,
        total: u64,
        progress: &mut dyn FnMut(u64, u64),
        cancel: &AtomicBool,
    ) -> Result<Downloaded> {
        let mut hasher = Context::new(&SHA256);
        let mut have = existing(path, &mut hasher, total)?;
        // 上次已经下完了：不用再问服务器（它会回 416）。
        if have == total && total > 0 {
            progress(have, total);
            return Ok(Downloaded {
                size: have,
                sha256: hex(hasher.finish().as_ref()),
            });
        }
        let mut request = self.agent.get(url);
        if have > 0 {
            request = request.header("Range", format!("bytes={have}-"));
        }
        let mut response = request.call().map_err(http)?;
        let status = response.status().as_u16();
        let resumed = status == 206 && have > 0;
        if !resumed {
            check_status(status)?;
            // 服务器从头发：丢掉已有的部分。
            have = 0;
            hasher = Context::new(&SHA256);
        }
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(resumed)
            .truncate(!resumed)
            .open(path)?;
        let mut body = response.body_mut().as_reader();
        let mut buf = vec![0u8; 64 * 1024];
        progress(have, total);
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(Error::Cancelled);
            }
            let n = body.read(&mut buf)?;
            if n == 0 {
                break;
            }
            have += n as u64;
            if have > total {
                return Err(Error::TooLarge);
            }
            file.write_all(&buf[..n])?;
            hasher.update(&buf[..n]);
            progress(have, total);
        }
        file.sync_all()?;
        Ok(Downloaded {
            size: have,
            sha256: hex(hasher.finish().as_ref()),
        })
    }
}

/// 已经下载的部分有多大，顺便把它算进 SHA-256。比预期还大的说明不是同一个文件，删掉。
fn existing(path: &Path, hasher: &mut Context, total: u64) -> Result<u64> {
    let Ok(mut file) = File::open(path) else {
        return Ok(0);
    };
    if file.metadata()?.len() > total {
        drop(file);
        std::fs::remove_file(path)?;
        return Ok(0);
    }
    let mut buf = vec![0u8; 64 * 1024];
    let mut have = 0;
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            return Ok(have);
        }
        hasher.update(&buf[..n]);
        have += n as u64;
    }
}

fn check_status(status: u16) -> Result<()> {
    if (200..300).contains(&status) {
        Ok(())
    } else {
        Err(Error::Http(format!("服务器返回 {status}")))
    }
}

fn http(e: ureq::Error) -> Error {
    match e {
        ureq::Error::Io(e) => Error::Io(e),
        e => Error::Http(e.to_string()),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
