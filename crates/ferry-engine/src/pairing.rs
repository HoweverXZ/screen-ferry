//! 配对：首次连接时让两边的用户核对同一个配对码，确认后互相记住对方的证书。
//!
//! 配对在单独的一条配对连接上进行，完成后即关闭。消息顺序与安全性论证见
//! [`ferry_trust::pairing`]。

use std::future::Future;
use std::time::Duration;

use ferry_net::Framed;
use ferry_proto::Message;
use ferry_trust::Fingerprint;
use ferry_trust::pairing::{self, Nonce, PairingCode};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::time::timeout;

use crate::error::{Error, Result};
use crate::link::send;

/// 从开始配对到双方都做出决定的时限，包括等用户确认的时间。
pub(crate) const PAIRING_TIMEOUT: Duration = Duration::from_secs(120);

/// 请用户核对配对码的方式，由应用提供：命令行里问一句，图形界面里弹个窗。
pub trait ConfirmPairing: Send + Sync + 'static {
    /// 显示配对码并等待用户决定。两台设备上显示的码相同、用户同意时返回 `true`。
    ///
    /// 对方先拒绝时，这个 future 会被直接丢弃。
    fn confirm(&self, request: &PairingRequest) -> impl Future<Output = bool> + Send;
}

/// 一次配对请求，交给用户核对。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PairingRequest {
    /// 对方的设备名。由对方自报，只用于显示。
    pub peer_name: String,
    pub peer_fingerprint: Fingerprint,
    /// 两台设备上应当显示的同一个配对码。
    pub code: PairingCode,
}

/// 配对的结局。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    Paired,
    RejectedHere,
    RejectedByPeer,
}

/// 作为接受方（服务端）配对：先承诺自己的随机数，收到对方的随机数后再公开。
pub(crate) async fn as_acceptor<S, P>(
    framed: &mut Framed<S>,
    local: Fingerprint,
    peer: Fingerprint,
    peer_name: &str,
    confirm: &P,
) -> Result<Outcome>
where
    S: AsyncRead + AsyncWrite + Unpin,
    P: ConfirmPairing,
{
    let run = async {
        let ours = pairing::nonce()?;
        let commitment = pairing::commit(&ours);
        send(framed, &Message::PairCommit { commitment }).await?;
        let theirs = expect_nonce(framed).await?;
        send(framed, &Message::PairNonce { nonce: ours }).await?;
        let code = pairing::code(&local, &peer, &ours, &theirs);
        decide(framed, request(peer_name, peer, code), confirm).await
    };
    timeout(PAIRING_TIMEOUT, run)
        .await
        .map_err(|_| Error::Timeout("配对"))?
}

/// 作为发起方（客户端）配对：收到承诺后发出自己的随机数，再核对对方公开的随机数。
pub(crate) async fn as_initiator<S, P>(
    framed: &mut Framed<S>,
    local: Fingerprint,
    peer: Fingerprint,
    peer_name: &str,
    confirm: &P,
) -> Result<Outcome>
where
    S: AsyncRead + AsyncWrite + Unpin,
    P: ConfirmPairing,
{
    let run = async {
        let commitment = match framed.recv().await? {
            Some(Message::PairCommit { commitment }) => commitment,
            _ => return Err(Error::PairingFailed("对方没有按顺序发送承诺")),
        };
        let ours = pairing::nonce()?;
        send(framed, &Message::PairNonce { nonce: ours }).await?;
        let theirs = expect_nonce(framed).await?;
        if !pairing::verify(&commitment, &theirs) {
            return Err(Error::PairingFailed(
                "对方公开的随机数与承诺不符，可能有人在中间篡改",
            ));
        }
        let code = pairing::code(&peer, &local, &theirs, &ours);
        decide(framed, request(peer_name, peer, code), confirm).await
    };
    timeout(PAIRING_TIMEOUT, run)
        .await
        .map_err(|_| Error::Timeout("配对"))?
}

fn request(peer_name: &str, peer: Fingerprint, code: PairingCode) -> PairingRequest {
    PairingRequest {
        peer_name: peer_name.to_owned(),
        peer_fingerprint: peer,
        code,
    }
}

async fn expect_nonce<S>(framed: &mut Framed<S>) -> Result<Nonce>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    match framed.recv().await? {
        Some(Message::PairNonce { nonce }) => Ok(nonce),
        _ => Err(Error::PairingFailed("对方没有按顺序发送随机数")),
    }
}

/// 等本机用户决定，同时留意对方的决定：任何一边拒绝就立即结束。
async fn decide<S, P>(
    framed: &mut Framed<S>,
    request: PairingRequest,
    confirm: &P,
) -> Result<Outcome>
where
    S: AsyncRead + AsyncWrite + Unpin,
    P: ConfirmPairing,
{
    let mut peer_accepted = false;
    let accepted = {
        let local = confirm.confirm(&request);
        tokio::pin!(local);
        loop {
            tokio::select! {
                accepted = &mut local => break accepted,
                message = framed.recv(), if !peer_accepted => match peer_result(message?)? {
                    true => peer_accepted = true,
                    false => return Ok(Outcome::RejectedByPeer),
                },
            }
        }
    };
    send(framed, &Message::PairResult { accepted }).await?;
    if !accepted {
        return Ok(Outcome::RejectedHere);
    }
    if !peer_accepted && !peer_result(framed.recv().await?)? {
        return Ok(Outcome::RejectedByPeer);
    }
    Ok(Outcome::Paired)
}

fn peer_result(message: Option<Message>) -> Result<bool> {
    match message {
        Some(Message::PairResult { accepted }) => Ok(accepted),
        Some(Message::Bye(_)) | None => Err(Error::PairingFailed("对方中途退出了配对")),
        Some(_) => Err(Error::PairingFailed("配对时收到了意外的消息")),
    }
}
