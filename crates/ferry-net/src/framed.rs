//! 帧读写：在任意字节流上收发协议消息。

use ferry_proto::{Channel, Codec, Message};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::error::{Error, Result};

/// 每次从流中至少预留这么多字节再读。
const READ_CHUNK: usize = 4 * 1024;

/// 一条连接上的消息收发。读写缓冲区复用，稳定运行时不再分配内存。
pub struct Framed<S> {
    io: S,
    codec: Codec,
    read_buf: Vec<u8>,
    write_buf: Vec<u8>,
}

impl<S: AsyncRead + AsyncWrite + Unpin> Framed<S> {
    pub fn new(io: S, channel: Channel) -> Self {
        Self {
            io,
            codec: Codec::new(channel),
            read_buf: Vec::with_capacity(READ_CHUNK),
            write_buf: Vec::with_capacity(READ_CHUNK),
        }
    }

    pub fn get_ref(&self) -> &S {
        &self.io
    }

    /// 发送一条消息。
    pub async fn send(&mut self, message: &Message) -> Result<()> {
        self.write_buf.clear();
        self.codec.encode(message, &mut self.write_buf)?;
        self.io.write_all(&self.write_buf).await?;
        self.io.flush().await?;
        Ok(())
    }

    /// 接收一条消息。对方在帧边界上正常关闭连接时返回 `None`。
    ///
    /// 可以放在 `select!` 中：被取消时已读到的数据保留在缓冲区里，不会丢失。
    pub async fn recv(&mut self) -> Result<Option<Message>> {
        loop {
            if let Some((message, used)) = self.codec.decode(&self.read_buf)? {
                self.read_buf.drain(..used);
                return Ok(Some(message));
            }
            self.read_buf.reserve(READ_CHUNK);
            if self.io.read_buf(&mut self.read_buf).await? == 0 {
                return if self.read_buf.is_empty() {
                    Ok(None)
                } else {
                    Err(Error::TruncatedFrame)
                };
            }
        }
    }
}
