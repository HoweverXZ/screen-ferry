//! 边缘投递：告诉对方“我在你的哪一边”，以及拖动中的提示，让对方同一位置提前滑出接收胶囊。

use ferry_types::Side;

use crate::error::{DecodeError, EncodeError};
use crate::limits::MAX_PATH_LEN;
use crate::wire::{Body, Reader, Writer};

/// 拖动中的提示。发送方拖着文件靠近相邻的边缘时发出，离开或放下后发 `Gone`。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DropHint {
    pub state: HintState,
    /// 沿着相邻那条边的位置：0 是一端，10000 是另一端（左右边从上往下，上下边从左往右）。
    pub along: u16,
    /// 有多靠近边缘，0–100。
    pub closeness: u8,
    /// 拖着的第一项的名字。
    pub name: String,
    /// 拖着几项。
    pub items: u32,
    /// 总字节数；还不知道时为 0。
    pub bytes: u64,
}

/// 拖动处在哪一步。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HintState {
    /// 离开了边缘，或者已经放下、取消：收起接收胶囊。
    Gone,
    /// 靠近边缘。
    Near,
    /// 到了胶囊上，松手就发送。
    Over,
}

impl Body for Side {
    fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        w.u8(match self {
            Self::Left => 1,
            Self::Right => 2,
            Self::Top => 3,
            Self::Bottom => 4,
        });
        Ok(())
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        match r.u8()? {
            1 => Ok(Self::Left),
            2 => Ok(Self::Right),
            3 => Ok(Self::Top),
            4 => Ok(Self::Bottom),
            _ => Err(DecodeError::InvalidValue("方向")),
        }
    }
}

impl Body for HintState {
    fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        w.u8(match self {
            Self::Gone => 0,
            Self::Near => 1,
            Self::Over => 2,
        });
        Ok(())
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        match r.u8()? {
            0 => Ok(Self::Gone),
            1 => Ok(Self::Near),
            2 => Ok(Self::Over),
            _ => Err(DecodeError::InvalidValue("拖动提示")),
        }
    }
}

impl Body for DropHint {
    fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        self.state.encode(w)?;
        if self.along > 10_000 {
            return Err(EncodeError::LimitExceeded("沿边位置"));
        }
        w.u16(self.along);
        if self.closeness > 100 {
            return Err(EncodeError::LimitExceeded("靠近程度"));
        }
        w.u8(self.closeness);
        w.str(&self.name, MAX_PATH_LEN, "文件名长度")?;
        w.u32(self.items);
        w.u64(self.bytes);
        Ok(())
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let state = HintState::decode(r)?;
        let along = r.u16()?;
        if along > 10_000 {
            return Err(DecodeError::InvalidValue("沿边位置"));
        }
        let closeness = r.u8()?;
        if closeness > 100 {
            return Err(DecodeError::InvalidValue("靠近程度"));
        }
        Ok(Self {
            state,
            along,
            closeness,
            name: r.str(MAX_PATH_LEN, "文件名长度")?,
            items: r.u32()?,
            bytes: r.u64()?,
        })
    }
}
