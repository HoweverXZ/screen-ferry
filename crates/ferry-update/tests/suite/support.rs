//! 测试用的发布密钥和清单。

use std::io::Cursor;

use ferry_update::{Asset, Manifest, Notes, SCHEMA};

/// 一把临时生成的发布密钥。
pub struct Key {
    pub public: String,
    secret: minisign::SecretKey,
}

impl Key {
    pub fn new() -> Self {
        let pair = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
        Self {
            public: pair.pk.to_base64(),
            secret: pair.sk,
        }
    }

    /// 给这些字节签名，返回 `.minisig` 文件的内容。
    pub fn sign(&self, data: &[u8]) -> String {
        minisign::sign(None, &self.secret, Cursor::new(data), None, None)
            .unwrap()
            .into_string()
    }
}

/// 一份 0.2.0 的清单：2026-11-20 发布，半年后过期。
pub fn manifest() -> Manifest {
    Manifest {
        schema: SCHEMA,
        version: "0.2.0".into(),
        published: "2026-11-20T08:00:00Z".into(),
        expires: "2027-05-19T08:00:00Z".into(),
        protocol: 1,
        notes: Notes {
            zh: "英文界面\n运行日志".into(),
            en: "English UI\nDiagnostics log".into(),
        },
        assets: [(
            "macos-universal".to_owned(),
            Asset {
                url: "https://example.com/download/v0.2.0/ScreenFerry-0.2.0-macos.zip".into(),
                size: 21_000_000,
                sha256: "ab".repeat(32),
            },
        )]
        .into(),
    }
}

/// 清单的 JSON 字节。
pub fn bytes(manifest: &Manifest) -> Vec<u8> {
    serde_json::to_vec_pretty(manifest).unwrap()
}
