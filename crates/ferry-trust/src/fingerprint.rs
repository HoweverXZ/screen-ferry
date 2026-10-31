//! 证书指纹：证书 DER 编码的 SHA-256。

use std::fmt;
use std::str::FromStr;

use ring::digest::{SHA256, digest};

use crate::error::Error;

/// 一台设备的身份标识。
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Fingerprint([u8; 32]);

impl Fingerprint {
    /// 计算一张证书（DER 编码）的指纹。
    pub fn of(cert_der: &[u8]) -> Self {
        let mut out = [0; 32];
        out.copy_from_slice(digest(&SHA256, cert_der).as_ref());
        Self(out)
    }

    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// 64 位小写十六进制，用于存盘。
    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// 便于人眼核对的短形式：前 8 字节，每 2 字节一组。
    pub fn short(&self) -> String {
        self.0[..8]
            .chunks(2)
            .map(|pair| format!("{:02X}{:02X}", pair[0], pair[1]))
            .collect::<Vec<_>>()
            .join("-")
    }
}

impl FromStr for Fingerprint {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self, Error> {
        let text = text.as_bytes();
        if text.len() != 64 {
            return Err(Error::InvalidFingerprint);
        }
        let mut out = [0; 32];
        for (byte, pair) in out.iter_mut().zip(text.chunks(2)) {
            let pair = std::str::from_utf8(pair).map_err(|_| Error::InvalidFingerprint)?;
            *byte = u8::from_str_radix(pair, 16).map_err(|_| Error::InvalidFingerprint)?;
        }
        Ok(Self(out))
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.short())
    }
}

impl fmt::Debug for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Fingerprint({})", self.short())
    }
}
