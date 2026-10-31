//! 传输：`127.0.0.1` 上的 TCP，每行一条 JSON；连接后先核对令牌和协议版本。

use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use ring::rand::{SecureRandom, SystemRandom};
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

use crate::endpoint::Endpoint;
use crate::error::{Error, Result};
use crate::message::{MAX_LINE, Notice, Request, VERSION};

/// 握手（第一条消息）的时限。
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(3);

/// 后台一侧：只监听本机回环地址。
pub struct Listener {
    inner: TcpListener,
    token: String,
}

impl Listener {
    /// 在 `127.0.0.1` 的随机端口上监听，生成新的令牌。返回的 [`Endpoint`] 交给界面。
    pub async fn bind() -> Result<(Self, Endpoint)> {
        let inner = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let token = random_token()?;
        let endpoint = Endpoint {
            port: inner.local_addr()?.port(),
            token: token.clone(),
            pid: std::process::id(),
            version: VERSION,
        };
        Ok((Self { inner, token }, endpoint))
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.inner.local_addr()?)
    }

    /// 接受一条连接。握手放在 [`Incoming::authenticate`] 里，调用方可以放到后台做，
    /// 一个迟迟不说话的连接不会挡住其他连接。
    pub async fn accept(&self) -> Result<Incoming> {
        loop {
            let (stream, addr) = self.inner.accept().await?;
            // 只监听了回环地址，这里只是多一道保险。
            if addr.ip().is_loopback() {
                return Ok(Incoming {
                    stream,
                    token: self.token.clone(),
                });
            }
        }
    }
}

/// 还没核对令牌的连接。
pub struct Incoming {
    stream: TcpStream,
    token: String,
}

impl Incoming {
    /// 等待对方的 `Hello`，核对令牌与协议版本，回复 `Welcome`。
    pub async fn authenticate(self) -> Result<Connection> {
        let mut conn = Connection::new(self.stream)?;
        let hello = timeout(HANDSHAKE_TIMEOUT, conn.recv::<Request>())
            .await
            .map_err(|_| Error::Timeout)??;
        match hello {
            Some(Request::Hello { token, version }) => {
                if !same(token.as_bytes(), self.token.as_bytes()) {
                    return Err(Error::BadToken);
                }
                if version != VERSION {
                    return Err(Error::Version {
                        ours: VERSION,
                        theirs: version,
                    });
                }
            }
            Some(_) => return Err(Error::Unexpected),
            None => return Err(Error::Closed),
        }
        conn.send(&Notice::Welcome { version: VERSION }).await?;
        Ok(conn)
    }
}

/// 界面（或另一份后台）一侧：连接后台并完成握手。
pub async fn connect(endpoint: &Endpoint) -> Result<Connection> {
    let stream = timeout(
        HANDSHAKE_TIMEOUT,
        TcpStream::connect((Ipv4Addr::LOCALHOST, endpoint.port)),
    )
    .await
    .map_err(|_| Error::Timeout)??;
    let mut conn = Connection::new(stream)?;
    conn.send(&Request::Hello {
        token: endpoint.token.clone(),
        version: VERSION,
    })
    .await?;
    match timeout(HANDSHAKE_TIMEOUT, conn.recv::<Notice>())
        .await
        .map_err(|_| Error::Timeout)??
    {
        Some(Notice::Welcome { version }) if version == VERSION => Ok(conn),
        Some(Notice::Welcome { version }) => Err(Error::Version {
            ours: VERSION,
            theirs: version,
        }),
        Some(_) => Err(Error::Unexpected),
        None => Err(Error::Closed),
    }
}

/// 一条已握手的连接。需要同时收发时用 [`Connection::split`] 拆开。
pub struct Connection {
    reader: Reader,
    writer: Writer,
}

impl Connection {
    fn new(stream: TcpStream) -> Result<Self> {
        stream.set_nodelay(true)?;
        let (read, write) = stream.into_split();
        Ok(Self {
            reader: Reader {
                inner: BufReader::new(read),
                line: Vec::new(),
            },
            writer: Writer { inner: write },
        })
    }

    pub async fn send<T: Serialize>(&mut self, message: &T) -> Result<()> {
        self.writer.send(message).await
    }

    pub async fn recv<T: DeserializeOwned>(&mut self) -> Result<Option<T>> {
        self.reader.recv().await
    }

    pub fn split(self) -> (Reader, Writer) {
        (self.reader, self.writer)
    }
}

pub struct Reader {
    inner: BufReader<OwnedReadHalf>,
    line: Vec<u8>,
}

impl Reader {
    /// 读下一条消息；对方正常关闭连接时为 `None`。
    pub async fn recv<T: DeserializeOwned>(&mut self) -> Result<Option<T>> {
        self.line.clear();
        let limit = MAX_LINE as u64 + 1;
        let read = (&mut self.inner)
            .take(limit)
            .read_until(b'\n', &mut self.line)
            .await?;
        if read == 0 {
            return Ok(None);
        }
        if self.line.last() != Some(&b'\n') {
            return Err(if self.line.len() > MAX_LINE {
                Error::LineTooLong
            } else {
                Error::Closed
            });
        }
        Ok(Some(serde_json::from_slice(&self.line)?))
    }
}

pub struct Writer {
    inner: OwnedWriteHalf,
}

impl Writer {
    pub async fn send<T: Serialize>(&mut self, message: &T) -> Result<()> {
        let mut line = serde_json::to_vec(message)?;
        if line.len() >= MAX_LINE {
            return Err(Error::LineTooLong);
        }
        line.push(b'\n');
        self.inner.write_all(&line).await?;
        Ok(())
    }
}

fn random_token() -> Result<String> {
    let mut bytes = [0u8; 32];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| std::io::Error::other("无法生成随机令牌"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// 逐字节比较且耗时与内容无关，不让对方靠计时猜令牌。
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}
