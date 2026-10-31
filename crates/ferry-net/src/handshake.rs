//! 握手流程：发起方发 `Hello`，接受方回 `Welcome` 或 `Reject`。

use ferry_proto::{Capabilities, Hello, Message, Welcome};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::error::{Error, Result};
use crate::framed::Framed;

/// 作为发起方握手。
pub async fn initiate<S>(framed: &mut Framed<S>, hello: Hello) -> Result<Welcome>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    framed.send(&Message::Hello(hello)).await?;
    match framed.recv().await? {
        Some(Message::Welcome(welcome)) => Ok(welcome),
        Some(Message::Reject(reject)) => Err(Error::Rejected(reject)),
        Some(_) => Err(Error::UnexpectedMessage {
            expected: "Welcome",
        }),
        None => Err(Error::Closed),
    }
}

/// 作为接受方握手。成功时返回对方的 `Hello` 和己方发出的 `Welcome`。
pub async fn accept<S>(
    framed: &mut Framed<S>,
    capabilities: Capabilities,
    session: u64,
    name: &str,
) -> Result<(Hello, Welcome)>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let hello = match framed.recv().await? {
        Some(Message::Hello(hello)) => hello,
        Some(_) => return Err(Error::UnexpectedMessage { expected: "Hello" }),
        None => return Err(Error::Closed),
    };
    match hello.accept(capabilities, session, name) {
        Ok(welcome) => {
            framed.send(&Message::Welcome(welcome.clone())).await?;
            Ok((hello, welcome))
        }
        Err(reject) => {
            framed.send(&Message::Reject(reject)).await?;
            Err(Error::Rejected(reject))
        }
    }
}
