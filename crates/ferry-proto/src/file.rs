//! 文件传输：数据连接上的控制消息，以及文件连接上的清单与内容。
//!
//! 发送方在数据连接上发 [`FileOffer`]，接收方回 [`FileAccept`]。随后由客户端打开一条文件连接，
//! 第一条消息是 [`FileOpen`]。文件连接上发送方先发清单（若干条 [`FileList`]），
//! 接收方回 [`FileResume`] 说明从哪里开始，发送方再按清单顺序发 [`FileData`]，
//! 每个文件以 [`FileEnd`] 结尾，全部收好后接收方回 [`FileDone`]。

use ferry_types::TransferId;

use crate::error::{DecodeError, EncodeError};
use crate::limits::{MAX_CHUNK_LEN, MAX_FILE_LIST, MAX_PATH_LEN};
use crate::transfer::CancelReason;
use crate::wire::{Body, Reader, Writer};

/// 提议发送一批文件。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileOffer {
    pub id: TransferId,
    /// 显示给对方的名字：只有一项时是它的名字，多项时是第一项的名字。
    pub name: String,
    /// 顶层项目数（用户拖的是几个文件或文件夹）。
    pub items: u32,
    /// 全部文件数，不含文件夹。
    pub files: u32,
    /// 全部文件的总字节数。
    pub bytes: u64,
}

/// 清单中的一个条目。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileEntry {
    /// 相对路径，以 `/` 分隔，第一段是顶层项目的名字。
    pub path: String,
    pub kind: EntryKind,
    /// 文件的字节数；文件夹为 0。
    pub size: u64,
    /// 修改时间，自 1970 年起的毫秒数；不知道时为 0。
    pub modified: u64,
}

/// 条目类型。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EntryKind {
    File,
    Dir,
}

/// 一段清单。最后一段 `last` 为真。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileList {
    /// 最多 [`MAX_FILE_LIST`] 个。
    pub entries: Vec<FileEntry>,
    pub last: bool,
}

/// 接收方说明从哪里开始：清单中第 `index` 个条目的第 `offset` 个字节。全新传输为 (0, 0)。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileResume {
    pub index: u32,
    pub offset: u64,
}

/// 一个文件发完，附上整个文件的 SHA-256。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileEnd {
    pub index: u32,
    pub sha256: [u8; 32],
}

/// 取消一次文件传输，收发双方都可以发出。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileCancel {
    pub id: TransferId,
    pub reason: CancelReason,
}

impl Body for TransferId {
    fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        w.array(&self.0);
        Ok(())
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self(r.array()?))
    }
}

impl Body for FileOffer {
    fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        self.id.encode(w)?;
        w.str(&self.name, MAX_PATH_LEN, "文件名长度")?;
        w.u32(self.items);
        w.u32(self.files);
        w.u64(self.bytes);
        Ok(())
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            id: TransferId::decode(r)?,
            name: r.str(MAX_PATH_LEN, "文件名长度")?,
            items: r.u32()?,
            files: r.u32()?,
            bytes: r.u64()?,
        })
    }
}

impl Body for EntryKind {
    fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        w.u8(match self {
            Self::File => 1,
            Self::Dir => 2,
        });
        Ok(())
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        match r.u8()? {
            1 => Ok(Self::File),
            2 => Ok(Self::Dir),
            _ => Err(DecodeError::InvalidValue("条目类型")),
        }
    }
}

impl Body for FileEntry {
    fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        w.str(&self.path, MAX_PATH_LEN, "路径长度")?;
        self.kind.encode(w)?;
        w.u64(self.size);
        w.u64(self.modified);
        Ok(())
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            path: r.str(MAX_PATH_LEN, "路径长度")?,
            kind: EntryKind::decode(r)?,
            size: r.u64()?,
            modified: r.u64()?,
        })
    }
}

impl Body for FileList {
    fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        w.list(&self.entries, MAX_FILE_LIST, "清单条目数")?;
        w.bool(self.last);
        Ok(())
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            entries: r.list(MAX_FILE_LIST, "清单条目数")?,
            last: r.bool()?,
        })
    }
}

impl Body for FileResume {
    fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        w.u32(self.index);
        w.u64(self.offset);
        Ok(())
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            index: r.u32()?,
            offset: r.u64()?,
        })
    }
}

impl Body for FileEnd {
    fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        w.u32(self.index);
        w.array(&self.sha256);
        Ok(())
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            index: r.u32()?,
            sha256: r.array()?,
        })
    }
}

impl Body for FileCancel {
    fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        self.id.encode(w)?;
        w.u16(self.reason.0);
        Ok(())
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            id: TransferId::decode(r)?,
            reason: CancelReason(r.u16()?),
        })
    }
}

/// 文件内容的一段，最多 [`MAX_CHUNK_LEN`] 字节。
pub(crate) fn encode_data(data: &[u8], w: &mut Writer<'_>) -> Result<(), EncodeError> {
    w.bytes(data, MAX_CHUNK_LEN, "分块大小")
}

pub(crate) fn decode_data(r: &mut Reader<'_>) -> Result<Vec<u8>, DecodeError> {
    r.bytes(MAX_CHUNK_LEN, "分块大小")
}
