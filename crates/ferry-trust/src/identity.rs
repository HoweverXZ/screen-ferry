//! 设备身份：一张自签名证书和它的私钥（ECDSA P-256）。

use std::fmt;

use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

use crate::error::{Error, Result};
use crate::fingerprint::Fingerprint;

/// 证书里的主机名。不用于校验，只为满足证书格式。
const SUBJECT_ALT_NAME: &str = "screenferry.local";

/// 本机的设备身份。
pub struct Identity {
    cert: CertificateDer<'static>,
    key: PrivatePkcs8KeyDer<'static>,
    fingerprint: Fingerprint,
}

impl Identity {
    /// 生成新的身份。证书的通用名是设备名，只用于显示。
    pub fn generate(name: &str) -> Result<Self> {
        let key = KeyPair::generate()?;
        let mut params = CertificateParams::new(vec![SUBJECT_ALT_NAME.to_owned()])?;
        let mut subject = DistinguishedName::new();
        subject.push(DnType::CommonName, name);
        params.distinguished_name = subject;
        let cert = params.self_signed(&key)?;
        Self::from_der(cert.der().to_vec(), key.serialize_der())
    }

    /// 从存盘的 DER 数据恢复：证书，以及 PKCS#8 格式的私钥。
    pub fn from_der(cert: Vec<u8>, key: Vec<u8>) -> Result<Self> {
        if cert.is_empty() {
            return Err(Error::InvalidIdentity("证书为空"));
        }
        // 只确认私钥能被解析，并且与证书的公钥对得上。
        let pair = KeyPair::try_from(key.as_slice())
            .map_err(|_| Error::InvalidIdentity("私钥无法解析"))?;
        if !contains(&cert, pair.public_key_raw()) {
            return Err(Error::InvalidIdentity("私钥与证书不匹配"));
        }
        Ok(Self {
            fingerprint: Fingerprint::of(&cert),
            cert: CertificateDer::from(cert),
            key: PrivatePkcs8KeyDer::from(key),
        })
    }

    pub fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }

    pub fn cert_der(&self) -> &[u8] {
        &self.cert
    }

    pub fn key_der(&self) -> &[u8] {
        self.key.secret_pkcs8_der()
    }

    /// 交给 rustls 的证书链（只有自己这一张）。
    pub fn cert_chain(&self) -> Vec<CertificateDer<'static>> {
        vec![self.cert.clone()]
    }

    /// 交给 rustls 的私钥。
    pub fn private_key(&self) -> PrivateKeyDer<'static> {
        PrivateKeyDer::Pkcs8(self.key.clone_key())
    }
}

impl Clone for Identity {
    fn clone(&self) -> Self {
        Self {
            cert: self.cert.clone(),
            key: self.key.clone_key(),
            fingerprint: self.fingerprint,
        }
    }
}

impl fmt::Debug for Identity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 不打印私钥。
        f.debug_struct("Identity")
            .field("fingerprint", &self.fingerprint)
            .finish_non_exhaustive()
    }
}

/// 证书 DER 中是否含有这段公钥。证书的公钥原样嵌在其中，不必完整解析 X.509。
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && haystack.windows(needle.len()).any(|w| w == needle)
}
