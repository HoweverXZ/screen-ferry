//! 屏幕切换。

use ferry_types::{LockKeys, Point};

use crate::error::{DecodeError, EncodeError};
use crate::wire::{Body, Reader, Writer};

/// 光标进入接收方的屏幕，此后的输入事件都发给它。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Enter {
    /// 进入序号，每次进入递增，对应的 `Leave` 带同一个序号。
    pub seq: u32,
    /// 光标进入的位置，使用接收方的桌面坐标系。
    pub position: Point,
    /// 发送方当前的锁定键状态，接收方据此同步。
    pub locks: LockKeys,
}

impl Body for Enter {
    fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        w.u32(self.seq);
        self.position.encode(w)?;
        self.locks.encode(w)
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            seq: r.u32()?,
            position: Point::decode(r)?,
            locks: LockKeys::decode(r)?,
        })
    }
}
