//! 帧编解码。
//!
//! 帧格式：`| 类型 u16 | 长度 u32 | 消息体 |`，全部小端，长度只计消息体。
//! 帧头一到就检查类型和长度，非法帧不必等消息体收完就能拒绝。

use crate::error::{DecodeError, EncodeError};
use crate::handshake::Channel;
use crate::limits::{MAX_DATA_PAYLOAD, MAX_INPUT_PAYLOAD};
use crate::message::{self, Message};
use crate::wire::{Reader, Writer};

/// 帧头字节数。
pub const HEADER_LEN: usize = 6;

/// 某一种连接上的编解码器。只负责字节与消息的转换，不持有缓冲区，也不做 IO。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Codec {
    channel: Channel,
}

impl Codec {
    pub const fn new(channel: Channel) -> Self {
        Self { channel }
    }

    pub const fn channel(self) -> Channel {
        self.channel
    }

    /// 本连接上单帧消息体的最大字节数。
    pub const fn max_payload(self) -> usize {
        match self.channel {
            Channel::Input => MAX_INPUT_PAYLOAD,
            Channel::Data => MAX_DATA_PAYLOAD,
        }
    }

    /// 把一帧追加到 `out` 末尾。失败时 `out` 保持原样。
    pub fn encode(self, message: &Message, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        let ty = message.type_id();
        if message::allowed(ty, self.channel) != Some(true) {
            return Err(EncodeError::WrongChannel(self.channel));
        }

        let start = out.len();
        out.extend_from_slice(&ty.to_le_bytes());
        out.extend_from_slice(&[0; 4]);

        let result = message.encode_body(&mut Writer::new(out)).and_then(|()| {
            let len = out.len() - start - HEADER_LEN;
            let max = self.max_payload();
            if len > max {
                return Err(EncodeError::FrameTooLarge { len, max });
            }
            out[start + 2..start + HEADER_LEN].copy_from_slice(&(len as u32).to_le_bytes());
            Ok(())
        });

        if result.is_err() {
            out.truncate(start);
        }
        result
    }

    /// 从 `buf` 开头解出一帧，返回消息和这一帧占用的字节数。
    ///
    /// 数据还不够一整帧时返回 `Ok(None)`，调用方继续读取后再试。
    pub fn decode(self, buf: &[u8]) -> Result<Option<(Message, usize)>, DecodeError> {
        let Some(header) = buf.get(..HEADER_LEN) else {
            return Ok(None);
        };
        let ty = u16::from_le_bytes([header[0], header[1]]);
        let len = u32::from_le_bytes([header[2], header[3], header[4], header[5]]) as usize;

        match message::allowed(ty, self.channel) {
            None => return Err(DecodeError::UnknownMessage(ty)),
            Some(false) => {
                return Err(DecodeError::WrongChannel {
                    ty,
                    channel: self.channel,
                });
            }
            Some(true) => {}
        }
        let max = self.max_payload();
        if len > max {
            return Err(DecodeError::FrameTooLarge { len, max });
        }

        let Some(payload) = buf.get(HEADER_LEN..HEADER_LEN + len) else {
            return Ok(None);
        };
        let mut reader = Reader::new(payload);
        let message = Message::decode_body(ty, &mut reader)?;
        reader.finish()?;
        Ok(Some((message, HEADER_LEN + len)))
    }
}
