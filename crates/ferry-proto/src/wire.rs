//! 线上编码的基本单元。
//!
//! 整数一律小端定长；布尔为 1 字节，只能是 0 或 1；字符串为 u16 长度 + UTF-8；
//! 字节串为 u32 长度 + 内容；列表为 u16 数量 + 元素。
//! 每个值只有一种合法编码，所以解码再编码必然得到相同的字节。

use ferry_types::{
    ClipboardFormat, Display, DisplayId, KeyAction, KeyCode, LockKeys, MouseButton, Point, Rect,
};

use crate::error::{DecodeError, EncodeError};

/// 能写入和读出线上格式的值。
pub(crate) trait Body: Sized {
    fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError>;
    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError>;
}

pub(crate) struct Writer<'a> {
    buf: &'a mut Vec<u8>,
}

impl<'a> Writer<'a> {
    pub(crate) fn new(buf: &'a mut Vec<u8>) -> Self {
        Self { buf }
    }

    pub(crate) fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }

    pub(crate) fn u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub(crate) fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub(crate) fn u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub(crate) fn i32(&mut self, v: i32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub(crate) fn bool(&mut self, v: bool) {
        self.u8(u8::from(v));
    }

    /// 定长字节串：长度由消息格式规定，不写长度前缀。
    pub(crate) fn array<const N: usize>(&mut self, v: &[u8; N]) {
        self.buf.extend_from_slice(v);
    }

    pub(crate) fn str(
        &mut self,
        v: &str,
        max: usize,
        what: &'static str,
    ) -> Result<(), EncodeError> {
        within(v.len(), max, what)?;
        self.u16(v.len() as u16);
        self.buf.extend_from_slice(v.as_bytes());
        Ok(())
    }

    pub(crate) fn bytes(
        &mut self,
        v: &[u8],
        max: usize,
        what: &'static str,
    ) -> Result<(), EncodeError> {
        within(v.len(), max, what)?;
        self.u32(v.len() as u32);
        self.buf.extend_from_slice(v);
        Ok(())
    }

    pub(crate) fn list<T: Body>(
        &mut self,
        items: &[T],
        max: usize,
        what: &'static str,
    ) -> Result<(), EncodeError> {
        within(items.len(), max, what)?;
        self.u16(items.len() as u16);
        items.iter().try_for_each(|item| item.encode(self))
    }
}

fn within(len: usize, max: usize, what: &'static str) -> Result<(), EncodeError> {
    if len > max {
        return Err(EncodeError::LimitExceeded(what));
    }
    Ok(())
}

pub(crate) struct Reader<'a> {
    buf: &'a [u8],
}

impl<'a> Reader<'a> {
    pub(crate) fn new(buf: &'a [u8]) -> Self {
        Self { buf }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], DecodeError> {
        let (head, rest) = self.buf.split_at_checked(n).ok_or(DecodeError::Truncated)?;
        self.buf = rest;
        Ok(head)
    }

    pub(crate) fn array<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        let mut out = [0; N];
        out.copy_from_slice(self.take(N)?);
        Ok(out)
    }

    pub(crate) fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take(1)?[0])
    }

    pub(crate) fn u16(&mut self) -> Result<u16, DecodeError> {
        self.array().map(u16::from_le_bytes)
    }

    pub(crate) fn u32(&mut self) -> Result<u32, DecodeError> {
        self.array().map(u32::from_le_bytes)
    }

    pub(crate) fn u64(&mut self) -> Result<u64, DecodeError> {
        self.array().map(u64::from_le_bytes)
    }

    pub(crate) fn i32(&mut self) -> Result<i32, DecodeError> {
        self.array().map(i32::from_le_bytes)
    }

    pub(crate) fn bool(&mut self) -> Result<bool, DecodeError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(DecodeError::InvalidValue("布尔值")),
        }
    }

    pub(crate) fn str(&mut self, max: usize, what: &'static str) -> Result<String, DecodeError> {
        let len = usize::from(self.u16()?);
        if len > max {
            return Err(DecodeError::LimitExceeded(what));
        }
        std::str::from_utf8(self.take(len)?)
            .map(str::to_owned)
            .map_err(|_| DecodeError::InvalidValue("UTF-8 字符串"))
    }

    pub(crate) fn bytes(&mut self, max: usize, what: &'static str) -> Result<Vec<u8>, DecodeError> {
        let len = self.u32()? as usize;
        if len > max {
            return Err(DecodeError::LimitExceeded(what));
        }
        Ok(self.take(len)?.to_vec())
    }

    pub(crate) fn list<T: Body>(
        &mut self,
        max: usize,
        what: &'static str,
    ) -> Result<Vec<T>, DecodeError> {
        let count = usize::from(self.u16()?);
        if count > max {
            return Err(DecodeError::LimitExceeded(what));
        }
        (0..count).map(|_| T::decode(self)).collect()
    }

    /// 消息体必须被恰好读完。
    pub(crate) fn finish(&self) -> Result<(), DecodeError> {
        if !self.buf.is_empty() {
            return Err(DecodeError::TrailingBytes);
        }
        Ok(())
    }
}

// ── ferry-types 中领域类型的编码 ──────────────────────────

impl Body for Point {
    fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        w.i32(self.x);
        w.i32(self.y);
        Ok(())
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            x: r.i32()?,
            y: r.i32()?,
        })
    }
}

impl Body for Rect {
    fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        w.i32(self.x);
        w.i32(self.y);
        w.u32(self.width);
        w.u32(self.height);
        Ok(())
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            x: r.i32()?,
            y: r.i32()?,
            width: r.u32()?,
            height: r.u32()?,
        })
    }
}

impl Body for Display {
    fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        w.u32(self.id.0);
        self.bounds.encode(w)?;
        w.u16(self.scale_percent);
        w.bool(self.primary);
        Ok(())
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            id: DisplayId(r.u32()?),
            bounds: Rect::decode(r)?,
            scale_percent: r.u16()?,
            primary: r.bool()?,
        })
    }
}

impl Body for KeyCode {
    fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        w.u32(self.raw());
        Ok(())
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        r.u32().map(Self::from_raw)
    }
}

impl Body for KeyAction {
    fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        w.u8(match self {
            Self::Down => 1,
            Self::Up => 2,
            Self::Repeat => 3,
        });
        Ok(())
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        match r.u8()? {
            1 => Ok(Self::Down),
            2 => Ok(Self::Up),
            3 => Ok(Self::Repeat),
            _ => Err(DecodeError::InvalidValue("按键动作")),
        }
    }
}

impl Body for MouseButton {
    fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        w.u8(self.number());
        Ok(())
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Self::new(r.u8()?).ok_or(DecodeError::InvalidValue("鼠标按键编号"))
    }
}

impl Body for LockKeys {
    fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        w.u8(self.bits());
        Ok(())
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Self::from_bits(r.u8()?).ok_or(DecodeError::InvalidValue("锁定键状态"))
    }
}

impl Body for ClipboardFormat {
    fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        w.u8(self.0);
        Ok(())
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        r.u8().map(Self)
    }
}
