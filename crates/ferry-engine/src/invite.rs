//! 邀请：服务端请一台客户端连过来。
//!
//! 被控制的那台电脑常常没有自己的键盘鼠标，没法在它上面输入地址；局域网发现又不总是可靠。
//! 所以控制的一方也能发起：连到对方的邀请端口（[`INVITE_PORT`]），用
//! `Hello { channel: Invite, session: 本机监听的端口 }` 告诉它来连哪里。对方回 `Welcome` 后关闭连接，
//! 再照常连过来；没配对时两边核对配对码，所以邀请本身不需要信任对方。

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use ferry_net::Tls;
use ferry_proto::{Capabilities, Channel, Hello, RejectReason};
use ferry_store::Store;
use ferry_trust::Fingerprint;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::time::timeout;
use tracing::{debug, info};

use crate::error::{Error, Result};
use crate::link::{CONNECT_TIMEOUT, HANDSHAKE_TIMEOUT};

/// 客户端等邀请的默认端口。
pub const INVITE_PORT: u16 = 24811;

/// 邀请 `addr` 上的客户端连到本机监听的 `port`。对方接受了返回它的指纹。
pub async fn invite(store: &Store, name: &str, addr: SocketAddr, port: u16) -> Result<Fingerprint> {
    let tls = Tls::new(&store.identity(name)?)?;
    let (mut framed, peer) = timeout(CONNECT_TIMEOUT, tls.connect(addr, Channel::Invite))
        .await
        .map_err(|_| Error::Timeout("连接"))??;
    let hello = Hello::new(
        Channel::Invite,
        Capabilities::empty(),
        u64::from(port),
        name,
    );
    timeout(HANDSHAKE_TIMEOUT, ferry_net::initiate(&mut framed, hello))
        .await
        .map_err(|_| Error::Timeout("握手"))??;
    Ok(peer)
}

/// 一个接受了的邀请：去连 `addr`，连上的必须是 `peer`（发邀请的那台）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Invitation {
    pub(crate) addr: SocketAddr,
    pub(crate) peer: Fingerprint,
}

/// 客户端：在 `listener` 上等邀请，接受的交给 `invitations`。
///
/// `current` 是本机此刻连着的服务端：正连着别的服务端时回 `Reject(BUSY)`；
/// 已经连着发邀请的这台时照样回 `Welcome`，但不必再连。
pub(crate) async fn listen(
    listener: TcpListener,
    tls: Tls,
    name: String,
    current: Arc<Mutex<Option<Fingerprint>>>,
    invitations: mpsc::UnboundedSender<Invitation>,
) {
    loop {
        let Ok((stream, from)) = listener.accept().await else {
            continue;
        };
        let (tls, name, current, invitations) = (
            tls.clone(),
            name.clone(),
            current.clone(),
            invitations.clone(),
        );
        tokio::spawn(async move {
            if let Err(e) = answer(stream, from, &tls, &name, &current, &invitations).await {
                debug!(%from, "邀请没有成：{e}");
            }
        });
    }
}

async fn answer(
    stream: TcpStream,
    from: SocketAddr,
    tls: &Tls,
    name: &str,
    current: &Mutex<Option<Fingerprint>>,
    invitations: &mpsc::UnboundedSender<Invitation>,
) -> Result<()> {
    let (mut framed, peer) = timeout(HANDSHAKE_TIMEOUT, tls.accept(stream))
        .await
        .map_err(|_| Error::Timeout("TLS 握手"))??;
    let connected = *current.lock().unwrap_or_else(|e| e.into_inner());
    let accepted = ferry_net::accept(&mut framed, Capabilities::empty(), name, |hello| {
        match (hello.channel, connected) {
            (Channel::Invite, Some(server)) if server != peer => Err(RejectReason::BUSY),
            (Channel::Invite, _) if port_of(hello.session).is_some() => Ok(0),
            _ => Err(RejectReason::UNKNOWN_SESSION),
        }
    });
    let (hello, _) = timeout(HANDSHAKE_TIMEOUT, accepted)
        .await
        .map_err(|_| Error::Timeout("握手"))??;
    let Some(port) = port_of(hello.session) else {
        return Ok(());
    };
    if connected == Some(peer) {
        return Ok(());
    }
    let addr = SocketAddr::new(from.ip(), port);
    info!(inviter = %hello.name, %addr, "收到邀请，去连它");
    let _ = invitations.send(Invitation { addr, peer });
    Ok(())
}

/// 邀请里带的端口；不是合法端口时为 `None`。
fn port_of(session: u64) -> Option<u16> {
    u16::try_from(session).ok().filter(|&p| p != 0)
}
