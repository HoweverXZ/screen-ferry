//! 配对码：在两台设备上显示同一个 6 位数字，由用户核对。
//!
//! 双方已经建立 TLS 连接，各自知道对方证书的指纹。流程（数字比较，带承诺）：
//!
//! ```text
//! 接受方                                   发起方
//!   ── PairCommit { commit(Na) } ─────────▶
//!   ◀──────────────── PairNonce { Nb } ──
//!   ── PairNonce { Na } ──────────────────▶  （发起方核对承诺）
//!        双方计算 code(指纹A, 指纹B, Na, Nb)，显示给用户核对
//! ```
//!
//! 中间人与两边分别建立连接时，两边看到的指纹不同，配对码一般也不同。
//! 承诺让中间人必须在看到对方随机数之前定下自己的随机数，
//! 无法反复试算去凑出两边相同的配对码，猜中的概率只有百万分之一。

use std::fmt;

use ring::digest::{Context, SHA256};
use ring::rand::{SecureRandom, SystemRandom};

use crate::error::{Error, Result};
use crate::fingerprint::Fingerprint;

/// 随机数与承诺的字节数，与协议中的 `PAIRING_NONCE_LEN` 一致。
pub const NONCE_LEN: usize = 32;

pub type Nonce = [u8; NONCE_LEN];

const COMMIT_DOMAIN: &[u8] = b"screenferry pairing commit v1";
const CODE_DOMAIN: &[u8] = b"screenferry pairing code v1";

/// 生成一个配对随机数。
pub fn nonce() -> Result<Nonce> {
    let mut nonce = [0; NONCE_LEN];
    SystemRandom::new()
        .fill(&mut nonce)
        .map_err(|_| Error::Random)?;
    Ok(nonce)
}

/// 对随机数的承诺。
pub fn commit(nonce: &Nonce) -> Nonce {
    hash(&[COMMIT_DOMAIN, nonce])
}

/// 公开的随机数是否与先前的承诺一致。
pub fn verify(commitment: &Nonce, nonce: &Nonce) -> bool {
    commit(nonce) == *commitment
}

/// 双方要核对的配对码。两边传入的参数相同（都按接受方、发起方的顺序）才会得到同一个码。
pub fn code(
    acceptor: &Fingerprint,
    initiator: &Fingerprint,
    acceptor_nonce: &Nonce,
    initiator_nonce: &Nonce,
) -> PairingCode {
    let digest = hash(&[
        CODE_DOMAIN,
        acceptor.as_bytes(),
        initiator.as_bytes(),
        acceptor_nonce,
        initiator_nonce,
    ]);
    let value = u32::from_le_bytes([digest[0], digest[1], digest[2], digest[3]]);
    PairingCode(value % 1_000_000)
}

fn hash(parts: &[&[u8]]) -> Nonce {
    let mut context = Context::new(&SHA256);
    for part in parts {
        // 每段前加长度，避免不同的拆分方式得到同一串字节。
        context.update(&(part.len() as u32).to_le_bytes());
        context.update(part);
    }
    let mut out = [0; NONCE_LEN];
    out.copy_from_slice(context.finish().as_ref());
    out
}

/// 6 位配对码，显示为 `123 456`。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PairingCode(u32);

impl PairingCode {
    pub const fn value(self) -> u32 {
        self.0
    }
}

impl fmt::Display for PairingCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:03} {:03}", self.0 / 1000, self.0 % 1000)
    }
}
