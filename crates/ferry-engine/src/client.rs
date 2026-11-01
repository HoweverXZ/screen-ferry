//! 客户端：被控制的一方。连接服务端，把收到的输入注入本机。
//!
//! 光标离开、连接断开或心跳超时时，都会立即松开所有按着的键，杜绝卡键。
//! 光标在服务端屏幕上时，本机光标隐藏起来；会话结束时恢复。
//!
//! 所有连接都走 TLS 1.3。服务端的证书不在信任列表里（第一次连接，或对方换了身份）时，
//! 先发起配对，由两边的用户核对配对码，确认后才建立会话。
//!
//! 双方都支持剪贴板时，握手后再连一条数据连接同步剪贴板；它失败不影响键鼠。
//!
//! 不指定地址时，每次连接前都在局域网里用 mDNS 寻找服务端（见 [`ferry_net::discovery`]），
//! 对方换了 IP 也能找回来。

use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ferry_core::{Pressed, bounds};
use ferry_net::discovery::{Browser, Lookup, pick};
use ferry_net::{ClientStream, Framed, Tls};
use ferry_platform::{Clipboard, Displays, Error as PlatformError, InputInject};
use ferry_proto::{Capabilities, Channel, Hello, Message, RejectReason};
use ferry_store::Store;
use ferry_trust::{Fingerprint, Peer};
use ferry_types::{Display, InputEvent, LockKeys, Point, Side};
use tokio::sync::watch;
use tokio::time::{MissedTickBehavior, interval, sleep, timeout};
use tracing::{debug, info, warn};

use crate::clipboard::{self, ClipLimit, ClipboardTask, Reporter, SharedClipboard, Start};
use crate::edge_drop::{Edge, EdgeFeed, gone};
use crate::error::{Error, Result};
use crate::events::{Event, Events, FoundServer};
use crate::files::{FileLink, Files, Opener};
use crate::link::{
    CONNECT_TIMEOUT, DISPLAY_CHECK, HANDSHAKE_TIMEOUT, capabilities, frame_of, primary_scale, send,
};
use crate::pairing::{self, ConfirmPairing, Outcome};

/// 两次连接尝试之间的间隔。
const RETRY_DELAY: Duration = Duration::from_secs(1);
/// 超过这么久收不到任何消息（服务端至少每 0.5 秒发一次 Ping），视为断线。
const SILENCE_TIMEOUT: Duration = Duration::from_secs(2);
/// 每轮在局域网里寻找服务端的时长。
const DISCOVERY_WAIT: Duration = Duration::from_secs(3);
/// 一台服务端有多个地址时，每个地址的连接时限。
const PER_ADDRESS_TIMEOUT: Duration = Duration::from_secs(2);
/// 客户端自己测量往返延迟的间隔（服务端的 Ping 只用于判断连接存活）。
const RTT_INTERVAL: Duration = Duration::from_secs(1);

/// 服务端在哪里。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServerAddr {
    /// 固定地址。
    Fixed(SocketAddr),
    /// 在局域网里用 mDNS 寻找：优先已配对的服务端；从未配对过时，局域网里只有一台才自动选它。
    Discover,
}

pub struct ClientConfig {
    pub server: ServerAddr,
    /// 本机设备名，显示给对方。
    pub name: String,
    /// 是否同步剪贴板。
    pub clipboard: bool,
    /// 剪贴板单项的大小上限，会话中途也能改。
    pub clipboard_limit: ClipLimit,
    /// 本机的快捷键以 Command 键为主（macOS）。告诉服务端，由它决定要不要互换 Command 与 Ctrl。
    pub command_key: bool,
    /// 数据目录：本机身份和已信任的设备。
    pub store: Store,
    /// 对外报告的事件。
    pub events: Events,
    /// 文件传输。跨会话保留，断线重连后接着传。
    pub files: Files,
    /// 边缘投递：报告本机光标和朝向服务端的那条边。
    pub edge: EdgeFeed,
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
    let finder = match config.server {
        ServerAddr::Fixed(addr) => Finder::Fixed(addr),
        ServerAddr::Discover => Finder::Discover(Browser::start()?),
    };
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
        events: config.events.clone(),
        clip_limit: config.clipboard_limit.clone(),
        in_session: false,
        cursor_hidden: false,
        edge: config.edge.clone(),
        peer_side: None,
        server: None,
    };
    let mut searching = false;
    let mut seen = Vec::new();
    loop {
        let target = match locate(&finder, &config, &mut searching, &mut seen).await {
            Ok(Some(target)) => target,
            Ok(None) => continue,
            Err(e @ Error::AmbiguousServers(_)) => return Err(e),
            Err(e) => {
                warn!("{e}");
                sleep(RETRY_DELAY).await;
                continue;
            }
        };
        let result = client.session(&config, &displays, &target).await;
        client.peer_side = None;
        client.edge.reset();
        client.release_all();
        client.hide_cursor(false);
        client.active.send_replace(false);
        if std::mem::take(&mut client.in_session)
            && let Some(peer) = client.server
        {
            let reason = result.as_ref().err().map(ToString::to_string);
            config.events.emit(Event::Disconnected { peer, reason });
        }
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

/// 这一次要连的服务端。
struct Target {
    addrs: Vec<SocketAddr>,
    /// 局域网广播中对方自称的指纹，连上后必须与证书一致。
    expected: Option<Fingerprint>,
}

/// 找服务端的方式。
enum Finder {
    Fixed(SocketAddr),
    /// 在后台持续收集局域网里的服务端。
    Discover(Browser),
}

/// 确定这一次连哪里。局域网里暂时没找到时返回 `None`，调用方稍后再试。
///
/// `seen` 是上次报告出去的局域网服务端列表，变了才再报告。
async fn locate(
    finder: &Finder,
    config: &ClientConfig,
    searching: &mut bool,
    seen: &mut Vec<FoundServer>,
) -> Result<Option<Target>> {
    let store = &config.store;
    let browser = match finder {
        Finder::Fixed(addr) => {
            info!(server = %addr, "正在连接服务端");
            return Ok(Some(Target {
                addrs: vec![*addr],
                expected: None,
            }));
        }
        Finder::Discover(browser) => browser,
    };

    let deadline = Instant::now() + DISCOVERY_WAIT;
    loop {
        let peers = store.peers()?;
        let found = browser.found();
        let list: Vec<FoundServer> = found
            .iter()
            .map(|d| FoundServer {
                name: d.name.clone(),
                fingerprint: d.fingerprint,
                addrs: d.addrs.clone(),
            })
            .collect();
        if list != *seen {
            config.events.emit(Event::Found(list.clone()));
            *seen = list;
        }
        match pick(&found, &peers) {
            Lookup::Trusted(server) | Lookup::Unpaired(server) => {
                *searching = false;
                info!(server = %server.name, addrs = ?server.addrs, "在局域网里找到服务端");
                return Ok(Some(Target {
                    addrs: server.addrs,
                    expected: Some(server.fingerprint),
                }));
            }
            Lookup::Nothing { others } if Instant::now() >= deadline => {
                if peers.is_empty() && others.len() > 1 {
                    let list = others
                        .iter()
                        .map(|s| format!("{}（{}）", s.name, s.addrs[0].ip()))
                        .collect::<Vec<_>>()
                        .join("、");
                    return Err(Error::AmbiguousServers(list));
                }
                if !*searching {
                    *searching = true;
                    config.events.emit(Event::Searching);
                    if peers.is_empty() {
                        info!("正在局域网里寻找服务端…");
                    } else {
                        info!("正在局域网里寻找已配对的服务端…");
                        if !others.is_empty() {
                            info!(
                                "局域网里另有 {} 台未配对的服务端；要与新的服务端配对，请用 connect <地址>",
                                others.len()
                            );
                        }
                    }
                }
                return Ok(None);
            }
            Lookup::Nothing { .. } => sleep(Duration::from_millis(200)).await,
        }
    }
}

/// 依次尝试各个地址，返回第一个连通的。
async fn connect_any(
    tls: &Tls,
    addrs: &[SocketAddr],
    channel: Channel,
) -> Result<(Framed<ClientStream>, Fingerprint, SocketAddr)> {
    let per = if addrs.len() > 1 {
        PER_ADDRESS_TIMEOUT
    } else {
        CONNECT_TIMEOUT
    };
    let mut last = Error::Timeout("连接");
    for &addr in addrs {
        match timeout(per, tls.connect(addr, channel)).await {
            Ok(Ok((framed, peer))) => return Ok((framed, peer, addr)),
            Ok(Err(e)) => last = e.into(),
            Err(_) => last = Error::Timeout("连接"),
        }
    }
    Err(last)
}

enum Ended {
    Session,
    Paired,
}

struct Client<I: InputInject, K, P> {
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
    events: Events,
    clip_limit: ClipLimit,
    /// 已建立会话、还没报告断开。
    in_session: bool,
    /// 本机光标是否被隐藏（光标在服务端屏幕上）。
    cursor_hidden: bool,
    /// 边缘投递：报告本机光标和朝向服务端的那条边。
    edge: EdgeFeed,
    /// 服务端在本机的哪一侧；服务端说了才知道。
    peer_side: Option<Side>,
    /// 最近一次会话的服务端。
    server: Option<Fingerprint>,
}

/// 无论因为什么停下（包括整个引擎被关掉），都松开对方按着的键、让本机光标重新出现。
impl<I: InputInject, K, P> Drop for Client<I, K, P> {
    fn drop(&mut self) {
        for event in self.pressed.release_all(self.cursor) {
            let _ = self.inject.inject(&event);
        }
        if self.cursor_hidden {
            let _ = self.inject.set_cursor_hidden(false);
        }
    }
}

impl<I: InputInject, K: Clipboard, P: ConfirmPairing> Client<I, K, P> {
    async fn session<D: Displays>(
        &mut self,
        config: &ClientConfig,
        displays: &D,
        target: &Target,
    ) -> Result<Ended> {
        if let Some(&addr) = target.addrs.first() {
            self.events.emit(Event::Connecting { addr });
        }
        let (mut framed, server, addr) =
            connect_any(&self.tls, &target.addrs, Channel::Input).await?;
        if target.expected.is_some_and(|expected| expected != server) {
            return Err(Error::AdvertisedMismatch);
        }
        if self.repair || !config.store.peers()?.contains(&server) {
            drop(framed);
            self.pair(config, addr, server).await?;
            self.repair = false;
            return Ok(Ended::Paired);
        }

        let caps = capabilities(config.clipboard, config.command_key);
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
        let mut screens = displays.displays()?;
        send(&mut framed, &Message::Displays(screens.clone())).await?;
        info!(server = %welcome.name, fingerprint = %server, "已连接，等待光标进入");
        self.in_session = true;
        self.server = Some(server);
        // 刚连上时光标在服务端屏幕上。
        self.hide_cursor(true);
        self.events.emit(Event::Connected {
            name: welcome.name.clone(),
            fingerprint: server,
            addr,
        });

        // 会话结束时随之丢弃，剪贴板同步也就停了。
        let clipboard = welcome.capabilities.contains(Capabilities::CLIPBOARD);
        let files = welcome.capabilities.contains(Capabilities::FILES);
        let edge_drop = welcome.capabilities.contains(Capabilities::EDGE_DROP);
        let link = Link {
            tls: self.tls.clone(),
            name: config.name.clone(),
            addr,
            caps,
            session: welcome.session,
            server,
        };
        let _data_task = if clipboard || files {
            match link.connect(Channel::Data).await {
                Ok(data) => {
                    info!("数据连接已建立");
                    let files = files.then(|| config.files.attach(server, Some(link.opener())));
                    Some(self.start_data(data, server, clipboard, files))
                }
                Err(e) => {
                    warn!("数据连接失败，本次不同步剪贴板、不能传文件：{e}");
                    None
                }
            }
        } else {
            None
        };

        // 自己定时 Ping 服务端测往返延迟；同时据最后一次收到消息的时刻判断断线。
        let epoch = Instant::now();
        let mut last_heard = Instant::now();
        let mut ping = interval(RTT_INTERVAL);
        ping.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut silence = interval(SILENCE_TIMEOUT / 4);
        silence.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut screen_check = interval(DISPLAY_CHECK);
        screen_check.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut hints = self.edge.hints();
        hints.mark_unchanged();
        loop {
            let message = tokio::select! {
                message = framed.recv() => message?,
                changed = hints.changed(), if edge_drop => {
                    if changed.is_ok() {
                        let hint = hints.borrow_and_update().clone();
                        let hint = hint.map_or_else(gone, |(_, hint)| hint);
                        send(&mut framed, &Message::DropHint(hint)).await?;
                    }
                    continue;
                }
                _ = screen_check.tick() => {
                    // 改了分辨率、缩放或插拔显示器：告诉服务端，它按新的范围移动光标。
                    // 查询偶尔失败（例如显示器正在切换）时等下一次。
                    if let Ok(now) = displays.displays()
                        && now != screens
                    {
                        info!("本机的显示器布局已更新，已告知服务端");
                        send(&mut framed, &Message::Displays(now.clone())).await?;
                        screens = now;
                        if let Some(side) = self.peer_side {
                            self.publish_edge(side, &screens, &welcome.name);
                        }
                    }
                    continue;
                }
                _ = ping.tick() => {
                    let nonce = epoch.elapsed().as_micros() as u64;
                    send(&mut framed, &Message::Ping { nonce }).await?;
                    continue;
                }
                _ = silence.tick() => {
                    if last_heard.elapsed() > SILENCE_TIMEOUT {
                        return Err(Error::HeartbeatTimeout);
                    }
                    continue;
                }
            };
            last_heard = Instant::now();
            match message {
                None | Some(Message::Bye(_)) => return Ok(Ended::Session),
                Some(Message::Ping { nonce }) => {
                    send(&mut framed, &Message::Pong { nonce }).await?
                }
                Some(Message::Pong { nonce }) => {
                    let sent = epoch + Duration::from_micros(nonce);
                    let now = Instant::now();
                    if sent <= now {
                        self.events.emit(Event::Rtt {
                            peer: server,
                            rtt: now - sent,
                        });
                    }
                }
                Some(Message::Enter(enter)) => {
                    // 先把光标移到入口再显示，免得它先在藏身处闪一下。
                    self.move_to(enter.position);
                    self.hide_cursor(false);
                    // 只同步 Caps Lock：对方可能没有 Num Lock（例如 Mac），同步它会误关本机的小键盘。
                    if let Err(e) = self.inject.sync_locks(enter.locks, LockKeys::CAPS) {
                        warn!("同步锁定键失败：{e}");
                    }
                    self.active.send_replace(true);
                    self.edge.pointer(|p| p.here = true);
                    info!(at = ?enter.position, "光标进入本机");
                    self.events.emit(Event::Cursor {
                        here: true,
                        hotkey: false,
                        peer: server,
                    });
                }
                Some(Message::Leave { .. }) => {
                    self.release_all();
                    self.hide_cursor(true);
                    self.active.send_replace(false);
                    self.edge.pointer(|p| p.here = false);
                    info!("光标离开本机");
                    self.events.emit(Event::Cursor {
                        here: false,
                        hotkey: false,
                        peer: server,
                    });
                }
                Some(Message::Input(event)) => self.apply(event),
                Some(Message::PeerSide { side }) => {
                    info!(?side, "服务端在本机的这一侧");
                    self.publish_edge(side, &screens, &welcome.name);
                }
                Some(Message::DropHint(hint)) => self.edge.remote(server, Some(hint)),
                Some(Message::Displays(_)) => {}
                Some(other) => debug!(?other, "忽略消息"),
            }
        }
    }

    /// 在单独的配对连接上与服务端核对配对码。任何一方拒绝都返回 [`Error::PairingRejected`]。
    ///
    /// `expected` 是刚才输入连接上看到的指纹：配对连接必须连到同一台设备。
    async fn pair(
        &mut self,
        config: &ClientConfig,
        addr: SocketAddr,
        expected: Fingerprint,
    ) -> Result<()> {
        let (mut framed, server) = timeout(CONNECT_TIMEOUT, self.tls.connect(addr, Channel::Pair))
            .await
            .map_err(|_| Error::Timeout("连接"))??;
        if server != expected {
            return Err(Error::PeerMismatch);
        }
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
                self.events.emit(Event::Paired {
                    name,
                    fingerprint: server,
                });
                Ok(())
            }
            Outcome::RejectedHere => {
                info!("已拒绝配对");
                self.events.emit(Event::PairingRejected {
                    name,
                    by_peer: false,
                });
                Err(Error::PairingRejected)
            }
            Outcome::RejectedByPeer => {
                info!(server = %name, "对方拒绝了配对");
                self.events.emit(Event::PairingRejected {
                    name,
                    by_peer: true,
                });
                Err(Error::PairingRejected)
            }
        }
    }

    fn start_data(
        &self,
        data: Framed<ClientStream>,
        server: Fingerprint,
        sync: bool,
        files: Option<FileLink>,
    ) -> ClipboardTask {
        let report = Reporter {
            peer: server,
            events: self.events.clone(),
            limit: self.clip_limit.clone(),
        };
        clipboard::spawn(
            data,
            self.clipboard.clone(),
            self.active.subscribe(),
            Start::Quiet,
            report,
            sync,
            files,
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
        let held = self.pressed.any_button();
        self.edge.pointer(|p| p.held = held);
        self.inject(&event);
    }

    fn move_to(&mut self, position: Point) {
        self.cursor = position;
        self.inject(&InputEvent::PointerMotion { position });
        self.edge.pointer(|p| {
            p.x = position.x as f32;
            p.y = position.y as f32;
        });
    }

    /// 告诉边缘投递：朝向服务端的是哪条边、本机屏幕多大。
    fn publish_edge(&mut self, side: Side, screens: &[Display], peer: &str) {
        self.peer_side = Some(side);
        let (Some(rect), Some(id)) = (bounds(screens), self.server) else {
            return;
        };
        self.edge.edges(vec![Edge {
            id,
            side,
            screen: frame_of(rect),
            scale: f32::from(primary_scale(screens)) / 100.0,
            peer: peer.to_owned(),
            online: true,
        }]);
    }

    /// 光标在服务端屏幕上时隐藏本机光标，回到本机时重新显示。失败只影响显示，不影响控制。
    fn hide_cursor(&mut self, hidden: bool) {
        if self.cursor_hidden == hidden {
            return;
        }
        self.cursor_hidden = hidden;
        if let Err(e) = self.inject.set_cursor_hidden(hidden) {
            debug!("{}本机光标失败：{e}", if hidden { "隐藏" } else { "显示" });
        }
    }

    fn release_all(&mut self) {
        for event in self.pressed.release_all(self.cursor) {
            self.inject(&event);
        }
        self.edge.pointer(|p| p.held = false);
    }

    fn inject(&mut self, event: &InputEvent) {
        match self.inject.inject(event) {
            Ok(()) => {}
            Err(e @ PlatformError::Unsupported(_)) => debug!("跳过：{e}"),
            Err(e) => warn!(?event, "注入失败：{e}"),
        }
    }
}

/// 本会话中再连一条数据连接或文件连接所需的信息。
#[derive(Clone)]
struct Link {
    tls: Tls,
    name: String,
    addr: SocketAddr,
    caps: Capabilities,
    session: u64,
    server: Fingerprint,
}

impl Link {
    /// 带着会话号连一条数据连接或文件连接，并确认连到的还是同一台服务端。
    async fn connect(&self, channel: Channel) -> Result<Framed<ClientStream>> {
        let handshake = async {
            let (mut framed, peer) = self.tls.connect(self.addr, channel).await?;
            if peer != self.server {
                return Err(Error::PeerMismatch);
            }
            let hello = Hello::new(channel, self.caps, self.session, self.name.clone());
            ferry_net::initiate(&mut framed, hello).await?;
            Ok::<_, Error>(framed)
        };
        timeout(CONNECT_TIMEOUT, handshake)
            .await
            .map_err(|_| Error::Timeout("建立连接"))?
    }

    /// 文件传输每次需要时调用，新开一条文件连接。
    fn opener(&self) -> Opener {
        let link = self.clone();
        Arc::new(move || {
            let link = link.clone();
            Box::pin(async move { link.connect(Channel::File).await })
        })
    }
}
