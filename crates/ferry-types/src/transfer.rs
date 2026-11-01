//! 文件传输的标识。

use std::fmt;
use std::str::FromStr;

/// 一次文件传输的编号，由发送方随机生成。
///
/// 会话中断后续传时沿用同一个编号，接收方据此找到已经收到的部分。
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TransferId(pub [u8; 16]);

impl fmt::Display for TransferId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|b| write!(f, "{b:02x}"))
    }
}

impl fmt::Debug for TransferId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TransferId({self})")
    }
}

/// [`TransferId`] 的文本形式不对。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParseTransferIdError;

impl fmt::Display for ParseTransferIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("传输号应为 32 个十六进制字符")
    }
}

impl std::error::Error for ParseTransferIdError {}

impl FromStr for TransferId {
    type Err = ParseTransferIdError;

    /// 读入 [`Display`](fmt::Display) 写出的 32 个十六进制字符，大小写均可。
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.as_bytes();
        if s.len() != 32 {
            return Err(ParseTransferIdError);
        }
        let mut out = [0u8; 16];
        for (i, pair) in s.chunks_exact(2).enumerate() {
            let hi = hex(pair[0]).ok_or(ParseTransferIdError)?;
            let lo = hex(pair[1]).ok_or(ParseTransferIdError)?;
            out[i] = hi << 4 | lo;
        }
        Ok(Self(out))
    }
}

fn hex(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}
