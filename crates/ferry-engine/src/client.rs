//! 客户端：被控制的一方。连接服务端，把收到的输入注入本机。
//!
//! 光标离开、连接断开或心跳超时时，都会立即松开所有按着的键，杜绝卡键。
//!
//! 所有连接都走 TLS 1.3。服务端的证书不在信任列表里（第一次连接，或对方换了身份）时，
//! 先发起配对，由两边的用户核对配对码，确认后才建立会话。
//!
//! 双方都支持剪贴板时，握手后再连一条数据连接同步剪贴板；它失败不影响键鼠。

use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ferry_core::Pressed;
use ferry_net::{ClientStream, Framed, Tls};
use ferry_platform::{Clipboard, Displays, Error as PlatformError, InputInject};
use ferry_proto::{Capabilities, Channel, Hello, Message, RejectReason};
use ferry_store::Store;
use ferry_trust::{Fingerprint, Peer};
use ferry_types::{InputEvent, LockKeys, Point};
use tokio::sync::watch;
use tokio::time::{sleep, timeout};
use tracing::{debug, info, warn};

use crate::clipboard::{self, ClipboardTask, SharedClipboard, Start};
use crate::error::{Error, Result};
use crate::link::{CONNECT_TIMEOUT, HANDSHAKE_TIMEOUT, capabilities, send};
use crate::pairing::{self, ConfirmPairing, Outcome};

/// 两次连接尝试之间的间隔。
const RETRY_DELAY: Duration = Duration::from_secs(1);
/// 超过这么久收不到任何消息（服务端至少每 0.5 秒发一次 Ping），视为断线。
const SILENCE_TIMEOUT: Duration = Duration::from_secs(2);

pub struct ClientConfig {
    pub server: SocketAddr,
    /// 本机设备名，显示给对方。
    pub name: String,
    /// 是否同步剪贴板。
    pub clipboard: bool,
    /// 数据目录：本机身份和已信任的设备。
    pub store: Store,
}

/// 运行客户端：连接服务端，断开后自动重连。
///
/// 需要配对时用 `confirm` 请本机用户核对配对码。
/// 启动失败（例如读不到本机显示器）或任何一方拒绝配对时返回错误。
pub async fn connect<I, D, K, P>(
    config: ClientConfig,
    inject: I,
    displays: D,
    clipboard: K,
    confirm: P,
) -> Result<Infallible>
where
    I: InputInject,
    D: Displays,
    K: Clipboard,
    P: ConfirmPairing,
{
    displays.displays()?;
    let identity = config.store.identity(&config.name)?;
    info!(fingerprint = %identity.fingerprint(), "本机身份");
    let mut client = Client {
        tls: Tls::new(&identity)?,
        fingerprint: identity.fingerprint(),
        confirm,
        repair: false,
        inject,
        clipboard: Arc::new(Mutex::new(clipboard)),
        active: watch::Sender::new(false),
        pressed: Pressed::new(),
        cursor: Point::default(),
    };
    loop {
        info!(server = %config.server, "正在连接服务端");
        let result = client.session(&config, &displays).await;
        client.release_all();
        client.active.send_replace(false);
        match result {
            // 刚配对完，立即用新的信任关系重连。
            Ok(Ended::Paired) => continue,
            Ok(Ended::Session) => info!("连接已结束"),
            Err(e @ Error::PairingRejected) => return Err(e),
            Err(e) => warn!("连接中断：{e}"),
        }
        sleep(RETRY_DELAY).await;
    }
}

enum Ended {
    Session,
    Paired,
}

struct Client<I, K, P> {
    tls: Tls,
    fingerprint: Fingerprint,
    confirm: P,
    /// 服务端说不认识本机：即使本机信任它，下次也要重新配对。
    repair: bool,
    inject: I,
    clipboard: SharedClipboard<K>,
    /// 光标是否在本机，供剪贴板同步判断谁是活动端。
    active: watch::Sender<bool>,
    pressed: Pressed,
    cursor: Point,
}

impl<I: InputInject, K: Clipboard, P: ConfirmPairing> Client<I, K, P> {
    async fn session<D: Displays>(&mut self, config: &ClientConfig, displays: &D) -> Result<Ended> {
        let (mut framed, server) = timeout(
            CONNECT_TIMEOUT,
            self.tls.connect(config.server, Channel::Input),
        )
        .await
        .map_err(|_| Error::Timeout("连接"))??;
        if self.repair || !config.store.peers()?.contains(&server) {
            drop(framed);
            self.pair(config).await?;
            self.repair = false;
            return Ok(Ended::Paired);
        }

        let caps = capabilities(config.clipboard);
        let hello = Hello::new(Channel::Input, caps, 0, config.name.clone());
        let welcome =
            match timeout(HANDSHAKE_TIMEOUT, ferry_net::initiate(&mut framed, hello)).await {
                Err(_) => return Err(Error::Timeout("握手")),
                Ok(Err(ferry_net::Error::Rejected(r))) if r.reason == RejectReason::NOT_PAIRED => {
                    warn!("服务端不再信任本机（可能删除了配对记录），将重新配对");
                    self.repair = true;
                    return Ok(Ended::Paired);
                }
                Ok(result) => result?,
            };
        send(&mut framed, &Message::Displays(displays.displays()?)).await?;
        info!(server = %welcome.name, fingerprint = %server, "已连接，等待光标进入");

        // 会话结束时随之丢弃，剪贴板同步也就停了。
        let _clipboard_task = if welcome.capabilities.contains(Capabilities::CLIPBOARD) {
            match connect_data(&self.tls, config, caps, welcome.session, server).await {
                Ok(data) => {
                    info!("数据连接已建立，开始同步剪贴板");
                    Some(self.sync_clipboard(data))
                }
                Err(e) => {
                    warn!("数据连接失败，本次不同步剪贴板：{e}");
                    None
                }
            }
        } else {
            None
        };

        loop {
            let message = timeout(SILENCE_TIMEOUT, framed.recv())
                .await
                .map_err(|_| Error::HeartbeatTimeout)??;
            match message {
                None | Some(Message::Bye(_)) => return Ok(Ended::Session),
                Some(Message::Ping { nonce }) => {
                    send(&mut framed, &Message::Pong { nonce }).await?
                }
                Some(Message::Enter(enter)) => {
                    self.move_to(enter.position);
                    // 只同步 Caps Lock：对方可能没有 Num Lock（例如 Mac），同步它会误关本机的小键盘。
                    if let Err(e) = self.inject.sync_locks(enter.locks, LockKeys::CAPS) {
                        warn!("同步锁定键失败：{e}");
                    }
                    self.active.send_replace(true);
                    info!(at = ?enter.position, "光标进入本机");
                }
                Some(Message::Leave { .. }) => {
                    self.release_all();
                    self.active.send_replace(false);
                    info!("光标离开本机");
                }
                Some(Message::Input(event)) => self.apply(event),
                Some(Message::Displays(_)) => {}
                Some(other) => debug!(?other, "忽略消息"),
            }
        }
    }

    /// 在单独的配对连接上与服务端核对配对码。任何一方拒绝都返回 [`Error::PairingRejected`]。
    async fn pair(&mut self, config: &ClientConfig) -> Result<()> {
        let (mut framed, server) = timeout(
            CONNECT_TIMEOUT,
            self.tls.connect(config.server, Channel::Pair),
        )
        .await
        .map_err(|_| Error::Timeout("连接"))??;
        let hello = Hello::new(Channel::Pair, Capabilities::empty(), 0, config.name.clone());
        let welcome = timeout(HANDSHAKE_TIMEOUT, ferry_net::initiate(&mut framed, hello))
            .await
            .map_err(|_| Error::Timeout("握手"))??;
        let name = welcome.name;
        info!(server = %name, fingerprint = %server, "这台服务端尚未配对，开始配对");

        let outcome =
            pairing::as_initiator(&mut framed, self.fingerprint, server, &name, &self.confirm)
                .await?;
        match outcome {
            Outcome::Paired => {
                let mut peers = config.store.peers()?;
                peers.add(Peer {
                    name: name.clone(),
                    fingerprint: server,
                });
                config.store.save_peers(&peers)?;
                info!(server = %name, "配对成功，已记住这台设备");
                Ok(())
            }
            Outcome::RejectedHere => {
                info!("已拒绝配对");
                Err(Error::PairingRejected)
            }
            Outcome::RejectedByPeer => {
                info!(server = %name, "对方拒绝了配对");
                Err(Error::PairingRejected)
            }
        }
    }

    fn sync_clipboard(&self, data: Framed<ClientStream>) -> ClipboardTask {
        clipboard::spawn(
            data,
            self.clipboard.clone(),
            self.active.subscribe(),
            Start::Quiet,
        )
    }

    fn apply(&mut self, event: InputEvent) {
        match event {
            InputEvent::PointerMotion { position } => return self.move_to(position),
            // 先把光标放到事件发生的位置，再执行按键或滚动。
            InputEvent::Button { position, .. } | InputEvent::Wheel { position, .. }
                if position != self.cursor =>
            {
                self.move_to(position);
            }
            _ => {}
        }
        self.pressed.track(&event);
        self.inject(&event);
    }

    fn move_to(&mut self, position: Point) {
        self.cursor = position;
        self.inject(&InputEvent::PointerMotion { position });
    }

    fn release_all(&mut self) {
        for event in self.pressed.release_all(self.cursor) {
            self.inject(&event);
        }
    }

    fn inject(&mut self, event: &InputEvent) {
        match self.inject.inject(event) {
            Ok(()) => {}
            Err(e @ PlatformError::Unsupported(_)) => debug!("跳过：{e}"),
            Err(e) => warn!(?event, "注入失败：{e}"),
        }
    }
}

/// 带着输入连接的会话号建立数据连接。对方必须与输入连接是同一台设备。
async fn connect_data(
    tls: &Tls,
    config: &ClientConfig,
    caps: Capabilities,
    session: u64,
    server: Fingerprint,
) -> Result<Framed<ClientStream>> {
    let handshake = async {
        let (mut framed, peer) = tls.connect(config.server, Channel::Data).await?;
        if peer != server {
            return Err(Error::PeerMismatch);
        }
        let hello = Hello::new(Channel::Data, caps, session, config.name.clone());
        ferry_net::initiate(&mut framed, hello).await?;
        Ok::<_, Error>(framed)
    };
    timeout(CONNECT_TIMEOUT, handshake)
        .await
        .map_err(|_| Error::Timeout("建立数据连接"))?
}
