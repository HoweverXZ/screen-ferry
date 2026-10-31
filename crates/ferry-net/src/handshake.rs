//! 握手流程：发起方发 `Hello`，接受方回 `Welcome` 或 `Reject`。

use ferry_proto::{Capabilities, Hello, Message, Reject, RejectReason, Welcome};
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
///
/// `session` 看过对方的 `Hello` 后给出本连接所属的会话号，或者拒绝的原因，
/// 例如数据连接带来的会话号不存在。版本不兼容时不会调用它。
pub async fn accept<S>(
    framed: &mut Framed<S>,
    capabilities: Capabilities,
    name: &str,
    session: impl FnOnce(&Hello) -> Result<u64, RejectReason>,
) -> Result<(Hello, Welcome)>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let hello = match framed.recv().await? {
        Some(Message::Hello(hello)) => hello,
        Some(_) => return Err(Error::UnexpectedMessage { expected: "Hello" }),
        None => return Err(Error::Closed),
    };
    let reply = hello
        .accept(capabilities, 0, name)
        .and_then(|welcome| match session(&hello) {
            Ok(session) => Ok(Welcome { session, ..welcome }),
            Err(reason) => Err(Reject::new(reason)),
        });
    match reply {
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
