//! TCP 连接。键鼠事件很小、很急，所以一律关闭 Nagle 算法。

use std::net::SocketAddr;

use ferry_proto::Channel;
use tokio::net::TcpStream;

use crate::error::Result;
use crate::framed::Framed;

/// 连接到 `addr`。
pub async fn connect(addr: SocketAddr, channel: Channel) -> Result<Framed<TcpStream>> {
    wrap(TcpStream::connect(addr).await?, channel)
}

/// 把已建立的 TCP 连接（例如 `accept` 得到的）包装成消息收发。
pub fn wrap(stream: TcpStream, channel: Channel) -> Result<Framed<TcpStream>> {
    stream.set_nodelay(true)?;
    Ok(Framed::new(stream, channel))
}
