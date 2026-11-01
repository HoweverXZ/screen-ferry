//! 服务端：拥有键鼠的一方。捕获本机键鼠，光标越过边缘后把输入发给客户端。
//!
//! 任何原因结束会话（对方断开、心跳超时、出错）时，都会立即把控制权交还本机。
//! 光标在对方屏幕时，按 Ctrl + Option（Alt）+ Esc 可以强制回到本机。
//!
//! 所有连接都走 TLS 1.3。只有配对过的设备能建立会话；陌生设备会先发起配对，
//! 由两边的用户核对配对码。客户端握手后会用拿到的会话号再连一条数据连接，
//! 用来同步剪贴板和收发文件控制消息；每次传文件再连一条文件连接。

use std::convert::Infallible;
use std::sync::{Arc, Mutex};

use ferry_core::{EdgeSwitch, KeyRemap, Pressed, Side, Transition, bounds};
use ferry_net::discovery::Advertiser;
use ferry_net::{Framed, ServerStream, Tls};
use ferry_platform::{CaptureEvent, Captured, Clipboard, Displays, InputCapture};
use ferry_proto::{
    Bye, ByeReason, Capabilities, Channel, Enter, Hello, Message, RejectReason, Welcome,
};
use ferry_store::Store;
use ferry_trust::{Fingerprint, Peer};
use ferry_types::{Display, InputEvent, KeyAction, KeyCode, Point};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{self, UnboundedReceiver};
use tokio::sync::watch;
use tokio::time::{MissedTickBehavior, interval, timeout};
use tracing::{debug, info, warn};

use crate::clipboard::{self, ClipboardTask, SharedClipboard, Start};
use crate::edge_drop::{Edge, EdgeFeed};
use crate::error::{Error, Result};
use crate::events::{Event, Events};
use crate::files::Files;
use crate::heartbeat::{self, Heartbeat};
use crate::link::{DISPLAY_CHECK, HANDSHAKE_TIMEOUT, capabilities, frame_of, primary_scale, send};
use crate::pairing::{self, ConfirmPairing, Outcome};

pub struct ServerConfig {
    /// 会话中途也能立即生效的设置。
    pub live: watch::Receiver<Live>,
    /// 本机设备名，显示给对方。
    pub name: String,
    /// 是否同步剪贴板。
    pub clipboard: bool,
    /// 数据目录：本机身份和已信任的设备。
    pub store: Store,
    /// 是否在局域网里用 mDNS 广播本机，让客户端不填地址也能找到。
    pub announce: bool,
    /// 对外报告的事件。
    pub events: Events,
    /// 文件传输。跨会话保留，断线重连后接着传。
    pub files: Files,
    /// 边缘投递：报告本机光标和朝向对方的那条边。
    pub edge: EdgeFeed,
}

/// 会话中途也能立即生效的设置。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Live {
    /// 客户端的屏幕位于本机的哪一侧。光标在对方屏幕时改变方向，会先回到本机。
    pub side: Side,
    /// 发送前对按键做的重映射，例如把 Command 当作对方的 Ctrl。
    pub remap: KeyRemap,
}

impl Live {
    /// 运行期间不会改变的设置。
    pub fn fixed(self) -> watch::Receiver<Self> {
        watch::channel(self).1
    }
}

/// 运行服务端：在 `listener` 上一次服务一个客户端，断开后继续等待下一个。
///
/// 陌生设备发起配对时，用 `confirm` 请本机用户核对配对码。
/// 只有启动失败（例如捕获权限不足、数据目录不可用）时才返回错误。
pub async fn serve<C, D, K, P>(
    listener: TcpListener,
    config: ServerConfig,
    capture: C,
    displays: D,
    clipboard: K,
    confirm: P,
) -> Result<Infallible>
where
    C: InputCapture,
    D: Displays,
    K: Clipboard,
    P: ConfirmPairing,
{
    let mut server = Server::start(config, capture, displays, clipboard, confirm)?;
    let addr = listener.local_addr()?;
    // 广播失败（例如网络不支持组播）不影响按地址连接。
    let advertiser = if server.config.announce {
        match Advertiser::start(&server.config.name, server.fingerprint, addr.port()) {
            Ok(advertiser) => Some(advertiser),
            Err(e) => {
                warn!("{e}；客户端需要手动指定本机地址");
                None
            }
        }
    } else {
        None
    };
    info!(
        %addr,
        fingerprint = %server.fingerprint,
        announced = advertiser.is_some(),
        "等待客户端连接",
    );
    server.config.events.emit(Event::Listening {
        port: addr.port(),
        announced: advertiser.is_some(),
    });

    let mut session = 0u64;
    loop {
        let accepted = tokio::select! {
            accepted = listener.accept() => accepted,
            // 没有客户端时，本机输入直接丢弃。
            _ = server.events.recv() => continue,
        };
        let (stream, peer) = match accepted {
            Ok(accepted) => accepted,
            Err(e) => {
                warn!("接受连接失败：{e}");
                continue;
            }
        };

        session += 1;
        let result = server.connection(stream, session, &listener).await;
        server.return_control();
        if std::mem::take(&mut server.in_session) {
            let reason = result.as_ref().err().map(ToString::to_string);
            server.config.events.emit(Event::Disconnected { reason });
        }
        match result {
            Ok(Ended::Session) => info!(%peer, "客户端已断开"),
            Ok(Ended::Other) => {}
            Err(e) => warn!(%peer, "连接中断：{e}"),
        }
    }
}

/// 一条新连接是怎么结束的。
enum Ended {
    /// 建立过会话。
    Session,
    /// 配对或被拒绝，没有建立会话。
    Other,
}

struct Server<C, D, K, P> {
    config: ServerConfig,
    capabilities: Capabilities,
    tls: Tls,
    fingerprint: Fingerprint,
    confirm: P,
    capture: C,
    displays: D,
    clipboard: SharedClipboard<K>,
    /// 光标是否在本机，供剪贴板同步判断谁是活动端。
    active: watch::Sender<bool>,
    events: UnboundedReceiver<Captured>,
    switch: Option<EdgeSwitch>,
    /// 按着的鼠标键和（光标在对方屏幕时）按着的键。
    /// 按着鼠标键时不切换屏幕，避免拖拽到一半换了屏；按着的键用来识别快捷键。
    pressed: Pressed,
    /// 已建立会话、还没报告断开。
    in_session: bool,
    /// 报告给边缘投递的那条边；会话中且对方能收文件时才有。
    edge: Option<Edge>,
    /// 对方懂边缘投递的消息（`PeerSide`、`DropHint`）。
    edge_drop: bool,
}

impl<C, D, K, P> Server<C, D, K, P>
where
    C: InputCapture,
    D: Displays,
    K: Clipboard,
    P: ConfirmPairing,
{
    fn start(
        config: ServerConfig,
        mut capture: C,
        displays: D,
        clipboard: K,
        confirm: P,
    ) -> Result<Self> {
        let identity = config.store.identity(&config.name)?;
        let tls = Tls::new(&identity)?;
        let (tx, events) = mpsc::unbounded_channel();
        capture.start(Box::new(move |event| {
            let _ = tx.send(event);
        }))?;
        Ok(Self {
            capabilities: capabilities(config.clipboard),
            tls,
            fingerprint: identity.fingerprint(),
            confirm,
            config,
            capture,
            displays,
            clipboard: Arc::new(Mutex::new(clipboard)),
            active: watch::Sender::new(true),
            events,
            switch: None,
            pressed: Pressed::new(),
            in_session: false,
            edge: None,
            edge_drop: false,
        })
    }

    /// 处理一条新连接：TLS 握手，然后按对方声明的连接类型配对或开始会话。
    async fn connection(
        &mut self,
        stream: TcpStream,
        id: u64,
        listener: &TcpListener,
    ) -> Result<Ended> {
        let addr = stream.peer_addr()?;
        let (mut framed, peer) = timeout(HANDSHAKE_TIMEOUT, self.tls.accept(stream))
            .await
            .map_err(|_| Error::Timeout("TLS 握手"))??;
        // 每次都从磁盘读：用户手动删掉某台设备后立即生效。
        let peers = self.config.store.peers()?;
        let handshake =
            ferry_net::accept(&mut framed, self.capabilities, &self.config.name, |hello| {
                match hello.channel {
                    Channel::Input if peers.contains(&peer) => Ok(id),
                    Channel::Input => Err(RejectReason::NOT_PAIRED),
                    Channel::Pair => Ok(id),
                    // 数据连接和文件连接只能在输入连接之后、带着它的会话号来。
                    Channel::Data | Channel::File => Err(RejectReason::UNKNOWN_SESSION),
                }
            });
        let (hello, welcome) = match timeout(HANDSHAKE_TIMEOUT, handshake).await {
            Err(_) => return Err(Error::Timeout("握手")),
            Ok(Err(ferry_net::Error::Rejected(r))) if r.reason == RejectReason::NOT_PAIRED => {
                info!(fingerprint = %peer, "一台未配对的设备想要连接，已请它先配对");
                return Ok(Ended::Other);
            }
            Ok(result) => result?,
        };

        if hello.channel == Channel::Pair {
            self.pair(&mut framed, peer, &hello.name).await?;
            return Ok(Ended::Other);
        }
        info!(peer = %hello.name, fingerprint = %peer, "客户端已连接");
        self.in_session = true;
        self.config.events.emit(Event::Connected {
            name: hello.name.clone(),
            fingerprint: peer,
            addr,
        });
        let result = self.session(framed, peer, hello, welcome, listener).await;
        self.edge = None;
        self.config.edge.edge(None);
        result?;
        Ok(Ended::Session)
    }

    async fn pair(
        &mut self,
        framed: &mut Framed<ServerStream>,
        peer: Fingerprint,
        name: &str,
    ) -> Result<()> {
        info!(peer = name, fingerprint = %peer, "收到配对请求");
        let outcome =
            pairing::as_acceptor(framed, self.fingerprint, peer, name, &self.confirm).await?;
        match outcome {
            Outcome::Paired => {
                let mut peers = self.config.store.peers()?;
                peers.add(Peer {
                    name: name.to_owned(),
                    fingerprint: peer,
                });
                self.config.store.save_peers(&peers)?;
                info!(peer = name, "配对成功，已记住这台设备");
                self.config.events.emit(Event::Paired {
                    name: name.to_owned(),
                    fingerprint: peer,
                });
            }
            Outcome::RejectedHere | Outcome::RejectedByPeer => {
                let by_peer = outcome == Outcome::RejectedByPeer;
                if by_peer {
                    info!(peer = name, "对方拒绝了配对");
                } else {
                    info!(peer = name, "已拒绝配对");
                }
                self.config.events.emit(Event::PairingRejected {
                    name: name.to_owned(),
                    by_peer,
                });
            }
        }
        Ok(())
    }

    async fn session(
        &mut self,
        mut framed: Framed<ServerStream>,
        peer: Fingerprint,
        hello: Hello,
        welcome: Welcome,
        listener: &TcpListener,
    ) -> Result<()> {
        // 等待连接（尤其是等用户确认配对）期间积压的本机输入与会话无关。
        while self.events.try_recv().is_ok() {}
        let id = welcome.session;

        let mut local = self.displays.displays()?;
        send(&mut framed, &Message::Displays(local.clone())).await?;
        let mut remote = timeout(HANDSHAKE_TIMEOUT, expect_displays(&mut framed))
            .await
            .map_err(|_| Error::Timeout("等待对方的显示器信息"))??;

        let local_rect = bounds(&local).ok_or(Error::NoDisplays("本机"))?;
        let remote_rect = bounds(&remote).ok_or(Error::NoDisplays("对方"))?;
        let scale = relative_scale(&local, &remote);
        let side = self.config.live.borrow_and_update().side;
        info!(
            peer = %hello.name,
            local = ?local_rect,
            remote = ?remote_rect,
            scale,
            "握手完成：把光标推出本机 {side:?} 侧边缘即可切换过去",
        );
        self.switch = Some(EdgeSwitch::new(local_rect, remote_rect, side, scale));
        if welcome.capabilities.contains(Capabilities::FILES) {
            self.edge = Some(Edge {
                side,
                screen: frame_of(local_rect),
                scale: f32::from(primary_scale(&local)) / 100.0,
                peer: hello.name.clone(),
            });
        }
        self.config.edge.edge(self.edge.clone());
        self.config.edge.pointer(|p| p.here = true);
        self.edge_drop = welcome.capabilities.contains(Capabilities::EDGE_DROP);
        if self.edge_drop {
            let side = side.opposite();
            send(&mut framed, &Message::PeerSide { side }).await?;
        }
        // 设置的发送方已经不在（命令行模式）时，不再等它的变化。
        let mut live_open = true;

        self.active.send_replace(true);
        let clipboard = welcome.capabilities.contains(Capabilities::CLIPBOARD);
        let files = welcome.capabilities.contains(Capabilities::FILES);
        let (data_tx, mut data_rx) = mpsc::channel(1);
        // 会话结束时随之丢弃，剪贴板同步和文件传输也就停了。
        let mut _clipboard_task: Option<ClipboardTask> = None;

        let mut heartbeat = Heartbeat::new();
        let mut tick = interval(heartbeat::TICK);
        tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut screens = interval(DISPLAY_CHECK);
        screens.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                accepted = listener.accept() => match accepted {
                    Ok((stream, addr)) => {
                        let accept = Accept { id, peer, clipboard, files, established: data_tx.clone() };
                        self.accept_data(stream, accept, addr);
                    }
                    Err(e) => warn!("接受连接失败：{e}"),
                },
                Some(data) = data_rx.recv() => {
                    info!("数据连接已建立");
                    let active = self.active.subscribe();
                    let task = clipboard::spawn(
                        data,
                        self.clipboard.clone(),
                        active,
                        Start::Announce,
                        self.config.events.clone(),
                        clipboard,
                        files.then(|| self.config.files.attach(None)),
                    );
                    _clipboard_task = Some(task);
                }
                event = self.events.recv() => {
                    let Some(event) = event else { return Ok(()) };
                    self.on_capture(event, &mut framed).await?;
                }
                changed = self.config.live.changed(), if live_open => {
                    if changed.is_err() {
                        live_open = false;
                        continue;
                    }
                    let side = self.config.live.borrow_and_update().side;
                    self.change_side(side, &mut framed).await?;
                }
                message = framed.recv() => match message? {
                    Some(Message::Pong { nonce }) => {
                        if let Some(rtt) = heartbeat.pong(nonce) {
                            self.config.events.emit(Event::Rtt(rtt));
                        }
                    }
                    Some(Message::Ping { nonce }) => send(&mut framed, &Message::Pong { nonce }).await?,
                    Some(Message::Displays(displays)) => {
                        remote = displays;
                        self.update_remote(&local, &remote);
                    }
                    Some(Message::Bye(_)) | None => return Ok(()),
                    Some(other) => debug!(?other, "忽略消息"),
                },
                _ = screens.tick() => self.refresh_local(&mut local, &remote, &mut framed).await?,
                _ = tick.tick() => {
                    if heartbeat.timed_out() {
                        let _ = send(&mut framed, &Message::Bye(Bye { reason: ByeReason::TIMEOUT })).await;
                        return Err(Error::HeartbeatTimeout);
                    }
                    if let Some(nonce) = heartbeat.poll_ping(self.is_remote()) {
                        send(&mut framed, &Message::Ping { nonce }).await?;
                    }
                    if let Some(report) = heartbeat.poll_report() {
                        info!("{report}");
                    }
                }
            }
        }
    }

    /// 会话期间到来的连接只能是本会话的数据连接。握手放到后台，不耽误键鼠转发。
    ///
    /// 数据连接和文件连接必须来自同一台设备（证书指纹相同），并带着本会话的会话号。
    /// 数据连接交给会话；文件连接交给文件传输。
    fn accept_data(&self, stream: TcpStream, accept: Accept, addr: std::net::SocketAddr) {
        let capabilities = self.capabilities;
        let name = self.config.name.clone();
        let tls = self.tls.clone();
        let files = self.config.files.clone();
        tokio::spawn(async move {
            let Accept {
                id,
                peer,
                clipboard,
                files: file_cap,
                established,
            } = accept;
            let handshake = async {
                let (mut framed, from) = tls.accept(stream).await?;
                let ours = |hello: &Hello| hello.session == id && from == peer;
                let (hello, _) = ferry_net::accept(&mut framed, capabilities, &name, |hello| {
                    match hello.channel {
                        Channel::Data if ours(hello) && (clipboard || file_cap) => Ok(id),
                        Channel::File if ours(hello) && file_cap => Ok(id),
                        Channel::Data | Channel::File => Err(RejectReason::UNKNOWN_SESSION),
                        Channel::Input | Channel::Pair => Err(RejectReason::BUSY),
                    }
                })
                .await?;
                Ok::<_, Error>((hello.channel, framed))
            };
            match timeout(HANDSHAKE_TIMEOUT, handshake).await {
                Ok(Ok((Channel::File, framed))) => files.serve(framed),
                Ok(Ok((_, framed))) => {
                    let _ = established.send(framed).await;
                }
                Ok(Err(e)) => warn!(%addr, "拒绝了一个连接：{e}"),
                Err(_) => warn!(%addr, "新连接握手超时"),
            }
        });
    }

    async fn on_capture(
        &mut self,
        captured: Captured,
        framed: &mut Framed<ServerStream>,
    ) -> Result<()> {
        let Some(switch) = self.switch.as_mut() else {
            return Ok(());
        };
        // 切换那一刻还在路上的事件，发生时光标在另一边：本机已经处理过它，或者它已被吞掉。
        // 按现在的状态解释它，会拿过时的位置判断边缘、把光标立刻推回去，所以不再使用；
        // 鼠标键照样记录，免得以为某个键一直按着。
        let current = captured.grabbed == switch.is_remote();

        let mut hotkey = false;
        let transition = match captured.event {
            CaptureEvent::Pointer { position, dx, dy } => {
                if !current {
                    return Ok(());
                }
                let free = !self.pressed.any_button();
                if !switch.is_remote() {
                    self.config.edge.pointer(|p| {
                        p.x = position.x as f32;
                        p.y = position.y as f32;
                        p.here = true;
                    });
                }
                if switch.is_remote() {
                    switch.remote_motion(dx, dy, free)
                } else if free {
                    switch.local_motion(position, dx, dy)
                } else {
                    None
                }
            }
            CaptureEvent::Button { button, pressed } => {
                let event = InputEvent::Button {
                    button,
                    pressed,
                    position: Point::default(),
                };
                self.pressed.track(&event);
                let held = self.pressed.any_button();
                self.config.edge.pointer(|p| p.held = held);
                if current && let Some(position) = switch.remote_position() {
                    let event = InputEvent::Button {
                        button,
                        pressed,
                        position,
                    };
                    send(framed, &Message::Input(event)).await?;
                }
                None
            }
            CaptureEvent::Wheel { dx, dy } => {
                if current && let Some(position) = switch.remote_position() {
                    send(
                        framed,
                        &Message::Input(InputEvent::Wheel { dx, dy, position }),
                    )
                    .await?;
                }
                None
            }
            CaptureEvent::Key { code, action } => {
                // 光标在本机时，按键由本机自己处理。
                if !switch.is_remote() {
                    return Ok(());
                }
                self.pressed.track(&InputEvent::Key { code, action });
                if action == KeyAction::Down
                    && code == KeyCode::ESCAPE
                    && escape_hotkey(&self.pressed)
                {
                    info!("快捷键 Ctrl + Option + Esc：回到本机");
                    hotkey = true;
                    switch.leave()
                } else {
                    let code = self.config.live.borrow().remap.apply(code);
                    send(framed, &Message::Input(InputEvent::Key { code, action })).await?;
                    None
                }
            }
        };

        self.perform(transition, framed, hotkey).await
    }

    /// 执行一次切换或移动。
    async fn perform(
        &mut self,
        transition: Option<Transition>,
        framed: &mut Framed<ServerStream>,
        hotkey: bool,
    ) -> Result<()> {
        match transition {
            Some(Transition::Enter { seq, at }) => {
                self.config.edge.pointer(|p| p.here = false);
                // 先抓取再通知对方：抓取失败时，会话结束后会照常归还控制权。
                self.capture.grab()?;
                let enter = Enter {
                    seq,
                    position: at,
                    locks: self.capture.lock_keys(),
                };
                send(framed, &Message::Enter(enter)).await?;
                self.active.send_replace(false);
                info!(?at, "光标进入对方屏幕");
                self.config.events.emit(Event::Cursor {
                    here: false,
                    hotkey: false,
                });
            }
            Some(Transition::Move(position)) => {
                send(
                    framed,
                    &Message::Input(InputEvent::PointerMotion { position }),
                )
                .await?;
            }
            Some(Transition::Leave { seq, local_at }) => {
                // 先归还本机控制再通知对方：发送失败也不会把本机键鼠锁住。
                self.capture.release(local_at)?;
                self.config.edge.pointer(|p| p.here = true);
                // 对方收到 Leave 会松开所有键，这些键不再需要跟踪。
                self.pressed.clear_keys();
                self.active.send_replace(true);
                send(framed, &Message::Leave { seq }).await?;
                info!("光标回到本机");
                self.config
                    .events
                    .emit(Event::Cursor { here: true, hotkey });
            }
            None => {}
        }
        Ok(())
    }

    /// 用户改了对方屏幕的方向。光标在对方屏幕时先回到本机。
    async fn change_side(&mut self, side: Side, framed: &mut Framed<ServerStream>) -> Result<()> {
        let Some(switch) = self.switch.as_mut() else {
            return Ok(());
        };
        if switch.side() == side {
            return Ok(());
        }
        let transition = switch.set_side(side);
        info!(?side, "对方屏幕的方向已改变");
        if let Some(edge) = &mut self.edge {
            edge.side = side;
            self.config.edge.edge(Some(edge.clone()));
        }
        if self.edge_drop {
            let side = side.opposite();
            send(framed, &Message::PeerSide { side }).await?;
        }
        self.perform(transition, framed, false).await
    }

    fn is_remote(&self) -> bool {
        self.switch.as_ref().is_some_and(EdgeSwitch::is_remote)
    }

    /// 本机的显示器布局变了：之后的进出按新的范围判断，并告诉对方。
    async fn refresh_local(
        &mut self,
        local: &mut Vec<Display>,
        remote: &[Display],
        framed: &mut Framed<ServerStream>,
    ) -> Result<()> {
        // 查询偶尔失败（例如显示器正在切换）时等下一次。
        let Ok(now) = self.displays.displays() else {
            return Ok(());
        };
        if now == *local {
            return Ok(());
        }
        let (Some(switch), Some(rect)) = (self.switch.as_mut(), bounds(&now)) else {
            return Ok(());
        };
        switch.set_local(rect, relative_scale(&now, remote));
        if let Some(edge) = &mut self.edge {
            edge.screen = frame_of(rect);
            edge.scale = f32::from(primary_scale(&now)) / 100.0;
            self.config.edge.edge(Some(edge.clone()));
        }
        info!(local = ?rect, "本机的显示器布局已更新");
        send(framed, &Message::Displays(now.clone())).await?;
        *local = now;
        Ok(())
    }

    fn update_remote(&mut self, local: &[Display], remote: &[Display]) {
        if let (Some(switch), Some(rect)) = (self.switch.as_mut(), bounds(remote)) {
            switch.set_remote(rect, relative_scale(local, remote));
            info!(remote = ?rect, "对方的显示器布局已更新");
        }
    }

    /// 光标在对方屏幕时，立即放回本机。
    fn return_control(&mut self) {
        self.pressed.clear_keys();
        self.active.send_replace(true);
        if let Some(at) = self.switch.take().as_mut().and_then(EdgeSwitch::abort) {
            match self.capture.release(at) {
                Ok(()) => info!("已把控制权交还本机"),
                Err(e) => warn!("交还本机控制失败：{e}"),
            }
        }
    }
}

/// 会话期间接受新连接所需的信息。
struct Accept {
    id: u64,
    peer: Fingerprint,
    /// 双方是否都开启了剪贴板同步。
    clipboard: bool,
    /// 双方是否都支持传文件。
    files: bool,
    /// 数据连接建好后交给会话。
    established: mpsc::Sender<Framed<ServerStream>>,
}

/// Ctrl + Option（Alt）是否都按着。左右两侧的键都算。
fn escape_hotkey(pressed: &Pressed) -> bool {
    let held = |a, b| pressed.is_held(a) || pressed.is_held(b);
    held(KeyCode::LEFT_CTRL, KeyCode::RIGHT_CTRL) && held(KeyCode::LEFT_ALT, KeyCode::RIGHT_ALT)
}

async fn expect_displays(framed: &mut Framed<ServerStream>) -> Result<Vec<Display>> {
    match framed.recv().await? {
        Some(Message::Displays(displays)) => Ok(displays),
        Some(_) => Err(Error::Protocol("握手后应先发送显示器信息")),
        None => Err(ferry_net::Error::Closed.into()),
    }
}

/// 本机移动一个坐标单位，对方移动多少个坐标单位（百分数）。
fn relative_scale(local: &[Display], remote: &[Display]) -> u16 {
    let ratio = u32::from(primary_scale(remote)) * 100 / u32::from(primary_scale(local));
    u16::try_from(ratio).unwrap_or(u16::MAX)
}
