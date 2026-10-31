//! 剪贴板：先通告，后按需拉取。
//!
//! 剪贴板变化时只发送 [`ClipboardOffer`]，说明有哪些格式、各多大；
//! 对方真正粘贴时才发 [`ClipboardRequest`]，内容以分块（`Chunk`）返回。

use ferry_types::ClipboardFormat;

use crate::error::{DecodeError, EncodeError};
use crate::limits::MAX_CLIPBOARD_FORMATS;
use crate::wire::{Body, Reader, Writer};

/// 剪贴板内容变化的通告。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClipboardOffer {
    /// 剪贴板版本号，每次变化递增。旧版本的请求会被取消。
    pub seq: u32,
    pub items: Vec<ClipboardItem>,
}

/// 通告中的一种格式。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClipboardItem {
    pub format: ClipboardFormat,
    /// 内容字节数，接收方据此决定是否拉取。
    pub size: u64,
}

/// 拉取某个版本、某种格式的剪贴板内容。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClipboardRequest {
    pub seq: u32,
    pub format: ClipboardFormat,
    /// 请求方分配的传输号，返回的分块带同一个号。
    pub transfer: u32,
}

impl Body for ClipboardItem {
    fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        self.format.encode(w)?;
        w.u64(self.size);
        Ok(())
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            format: ClipboardFormat::decode(r)?,
            size: r.u64()?,
        })
    }
}

impl Body for ClipboardOffer {
    fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        w.u32(self.seq);
        w.list(&self.items, MAX_CLIPBOARD_FORMATS, "剪贴板格式数量")
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            seq: r.u32()?,
            items: r.list(MAX_CLIPBOARD_FORMATS, "剪贴板格式数量")?,
        })
    }
}

impl Body for ClipboardRequest {
    fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        w.u32(self.seq);
        self.format.encode(w)?;
        w.u32(self.transfer);
        Ok(())
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            seq: r.u32()?,
            format: ClipboardFormat::decode(r)?,
            transfer: r.u32()?,
        })
    }
}
