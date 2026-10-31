//! 分块传输：剪贴板内容（以后还有文件）按块发送，可随时取消。

use crate::error::{DecodeError, EncodeError};
use crate::limits::MAX_CHUNK_LEN;
use crate::wire::{Body, Reader, Writer};

/// 一个数据分块。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chunk {
    pub transfer: u32,
    /// 是否为最后一块。
    pub last: bool,
    /// 最多 [`MAX_CHUNK_LEN`] 字节。
    pub data: Vec<u8>,
}

/// 取消一次传输，收发双方都可以发出。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cancel {
    pub transfer: u32,
    pub reason: CancelReason,
}

/// 取消原因。以后可能新增，所以用编号表示。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CancelReason(pub u16);

impl CancelReason {
    /// 用户或程序主动取消。
    pub const CANCELLED: Self = Self(0);
    /// 内容已过期：剪贴板又变了。
    pub const STALE: Self = Self(1);
    /// 超过接收方允许的大小。
    pub const TOO_LARGE: Self = Self(2);
    /// 源数据已不可用。
    pub const UNAVAILABLE: Self = Self(3);
}

impl Body for Chunk {
    fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        w.u32(self.transfer);
        w.bool(self.last);
        w.bytes(&self.data, MAX_CHUNK_LEN, "分块大小")
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            transfer: r.u32()?,
            last: r.bool()?,
            data: r.bytes(MAX_CHUNK_LEN, "分块大小")?,
        })
    }
}

impl Body for Cancel {
    fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        w.u32(self.transfer);
        w.u16(self.reason.0);
        Ok(())
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            transfer: r.u32()?,
            reason: CancelReason(r.u16()?),
        })
    }
}
