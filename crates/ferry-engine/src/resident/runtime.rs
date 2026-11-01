//! 常驻运行的状态：引擎的启停、引擎事件与界面命令的处理、给界面的状态快照。

use std::collections::{BTreeMap, HashMap};
use std::convert::Infallible;
use std::io::ErrorKind;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use ferry_core::KeyRemap;
use ferry_ipc::{
    ClipboardSync, FoundDevice, Link, LinkStatus, Local, LogIcon, LogKind, Notice, PairingPrompt,
    PeerInfo, PeerRef, Problem, ProblemKind, Request, Settings, SettingsPatch, State,
};
use ferry_store::Store;
use ferry_trust::{Fingerprint, PeerList};
use ferry_types::{Config, Role, Side};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;
use tracing::{info, warn};

use super::confirm::{PairAsk, UiConfirm};
use super::record::{Activity, Log, Rtt, now_ms};
use super::ui::UiEvent;
use super::{Command, Host, Summary};
use crate::client::{ClientConfig, ServerAddr, connect};
use crate::edge_drop::EdgeDrop;
use crate::error::{Error, Result};
use crate::events::{Event, Events, FoundServer, TransferState, TransferUpdate};
use crate::files::Files;
use crate::link::DEFAULT_PORT;
use crate::server::{Live, ServerConfig, serve};

/// 缺少权限时多久再试一次：用户在系统设置里勾上之后很快就能用。
const RETRY_PERMISSION: Duration = Duration::from_secs(2);
/// 配对被拒绝后多久再试。
const RETRY_PAIRING: Duration = Duration::from_secs(3);
/// 其他问题多久再试。
const RETRY_OTHER: Duration = Duration::from_secs(5);
/// 多久重新读一次已配对设备（用户可能用命令行删了设备）。
const PEERS_REFRESH: Duration = Duration::from_secs(5);

struct EngineTask {
    handle: JoinHandle<Result<Infallible>>,
    /// 服务端的即时设置；客户端没有。
    live: Option<watch::Sender<Live>>,
}

struct Pending {
    prompt: PairingPrompt,
    reply: oneshot::Sender<bool>,
}

pub(crate) struct Runtime<H> {
    store: Store,
    host: H,
    config: Config,
    name: String,
    fingerprint: Fingerprint,
    events: Events,
    asks: mpsc::UnboundedSender<PairAsk>,
    /// 文件传输。引擎重启后仍保留，断线重连后接着传。
    files: Files,
    /// 边缘投递胶囊。
    edge: EdgeDrop,

    paused: bool,
    engine: Option<EngineTask>,
    retry_at: Option<Instant>,
    problem: Option<Problem>,

    status: LinkStatus,
    /// 服务端正在监听的端口。
    port: Option<u16>,
    peer: Option<PeerRef>,
    peer_fingerprint: Option<Fingerprint>,
    since: Option<i64>,
    cursor_on_peer: bool,
    rtt: Rtt,
    clipboard: Option<ClipboardSync>,
    peers: PeerList,
    peers_read: Instant,
    last_seen: HashMap<Fingerprint, i64>,
    found: Vec<FoundServer>,
    autostart: bool,

    log: Log,
    activity: Activity,
    last_second: Instant,

    clients: BTreeMap<u64, mpsc::Sender<Notice>>,
    next_client: u64,
    pending: BTreeMap<u64, Pending>,
    next_prompt: u64,
    dirty: bool,
}

impl<H: Host> Runtime<H> {
    pub(crate) fn new(
        store: Store,
        host: H,
        events: Events,
        asks: mpsc::UnboundedSender<PairAsk>,
    ) -> Result<Self> {
        let config = store.config()?;
        let name = resolve_name(&config, &host);
        let fingerprint = store.identity(&name)?.fingerprint();
        let peers = store.peers()?;
        let autostart = host.autostart();
        let files = Files::new(&store, events.clone());
        let edge = EdgeDrop::spawn(host.overlay(), files.clone(), accent_color(config.accent));
        let mut runtime = Self {
            store,
            host,
            config,
            name,
            fingerprint,
            events,
            asks,
            files,
            edge,
            paused: false,
            engine: None,
            retry_at: None,
            problem: None,
            status: LinkStatus::Starting,
            port: None,
            peer: None,
            peer_fingerprint: None,
            since: None,
            cursor_on_peer: false,
            rtt: Rtt::default(),
            clipboard: None,
            peers,
            peers_read: Instant::now(),
            last_seen: HashMap::new(),
            found: Vec::new(),
            autostart,
            log: Log::default(),
            activity: Activity::default(),
            last_second: Instant::now(),
            clients: BTreeMap::new(),
            next_client: 1,
            pending: BTreeMap::new(),
            next_prompt: 1,
            dirty: true,
        };
        let role = match runtime.config.role {
            Role::Server => "键鼠在本机",
            Role::Client => "由另一台电脑控制",
        };
        runtime.log(
            LogKind::Link,
            LogIcon::Power,
            "ScreenFerry 已启动",
            format!("{} · {role}", runtime.name),
        );
        Ok(runtime)
    }

    pub(crate) fn next_client_id(&mut self) -> u64 {
        let id = self.next_client;
        self.next_client += 1;
        id
    }

    // ── 引擎 ─────────────────────────────────────

    pub(crate) fn start_engine(&mut self) {
        if self.paused || self.engine.is_some() {
            return;
        }
        self.retry_at = None;
        self.status = LinkStatus::Starting;
        self.dirty = true;
        let confirm = UiConfirm {
            asks: self.asks.clone(),
        };
        let started = match self.config.role {
            Role::Server => self.spawn_server(confirm),
            Role::Client => self.spawn_client(confirm),
        };
        if let Err(e) = started {
            self.fail(e);
        }
    }

    fn spawn_server(&mut self, confirm: UiConfirm) -> Result<()> {
        self.host.check_permissions(Role::Server)?;
        let capture = self.host.capture()?;
        let (live_tx, live_rx) = watch::channel(self.live());
        let config = ServerConfig {
            live: live_rx,
            name: self.name.clone(),
            clipboard: self.config.clipboard,
            store: self.store.clone(),
            announce: self.config.announce,
            events: self.events.clone(),
            files: self.files.clone(),
            edge: self.edge.feed(),
        };
        let (displays, clipboard) = (self.host.displays(), self.host.clipboard());
        let port = self.host.listen_port();
        let handle = tokio::spawn(async move {
            let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, port)).await?;
            serve(listener, config, capture, displays, clipboard, confirm).await
        });
        self.engine = Some(EngineTask {
            handle,
            live: Some(live_tx),
        });
        Ok(())
    }

    fn spawn_client(&mut self, confirm: UiConfirm) -> Result<()> {
        self.host.check_permissions(Role::Client)?;
        let inject = self.host.inject()?;
        let server = match self.config.server.trim() {
            "" => ServerAddr::Discover,
            text => ServerAddr::Fixed(resolve(text)?),
        };
        let config = ClientConfig {
            server,
            name: self.name.clone(),
            clipboard: self.config.clipboard,
            store: self.store.clone(),
            events: self.events.clone(),
            files: self.files.clone(),
            edge: self.edge.feed(),
        };
        let (displays, clipboard) = (self.host.displays(), self.host.clipboard());
        let handle = tokio::spawn(connect(config, inject, displays, clipboard, confirm));
        self.engine = Some(EngineTask { handle, live: None });
        Ok(())
    }

    /// 停下引擎：丢弃它的任务，引擎随之归还鼠标控制、松开按键、断开连接。
    pub(crate) async fn stop_engine(&mut self) {
        if let Some(task) = self.engine.take() {
            task.handle.abort();
            let _ = task.handle.await;
        }
        self.drop_link();
        self.retry_at = None;
    }

    async fn restart_engine(&mut self) {
        self.stop_engine().await;
        self.start_engine();
    }

    /// 等待引擎自己结束（只会因为出错）。没有在运行的引擎时永远不返回。
    pub(crate) async fn engine_exit(&mut self) -> Result<Infallible> {
        let Some(task) = self.engine.as_mut() else {
            return std::future::pending().await;
        };
        let result = (&mut task.handle).await;
        self.engine = None;
        match result {
            Ok(result) => result,
            Err(e) => Err(Error::Io(std::io::Error::other(format!(
                "引擎意外退出：{e}"
            )))),
        }
    }

    pub(crate) fn on_engine_exit(&mut self, result: Result<Infallible>) {
        let Err(e) = result;
        warn!("引擎停止：{e}");
        self.drop_link();
        self.fail(e);
    }

    /// 引擎无法工作：记下问题，过一会儿再试。
    fn fail(&mut self, e: Error) {
        self.dirty = true;
        let (kind, delay) = match &e {
            Error::Platform(ferry_platform::Error::PermissionDenied(_)) => {
                (ProblemKind::Permission, RETRY_PERMISSION)
            }
            Error::Io(io) if io.kind() == ErrorKind::AddrInUse => {
                (ProblemKind::PortInUse, RETRY_OTHER)
            }
            Error::PairingRejected => {
                // 不是故障：用户或对方拒绝了配对，稍后照常寻找。
                self.status = LinkStatus::Searching;
                self.retry_at = Some(Instant::now() + RETRY_PAIRING);
                return;
            }
            _ => (ProblemKind::Other, RETRY_OTHER),
        };
        let message = match kind {
            ProblemKind::Permission => {
                "需要在系统设置的“隐私与安全性”中允许 ScreenFerry 控制键盘和鼠标".to_owned()
            }
            ProblemKind::PortInUse => format!(
                "端口 {} 被占用：可能还开着 Deskflow 或另一份 ScreenFerry",
                self.host.listen_port()
            ),
            ProblemKind::Other => e.to_string(),
        };
        if self.problem.as_ref().is_none_or(|p| p.message != message) {
            self.log(
                LogKind::Link,
                LogIcon::Warning,
                "无法开始共享",
                message.clone(),
            );
        }
        self.problem = Some(Problem { kind, message });
        self.status = LinkStatus::Stopped;
        self.retry_at = Some(Instant::now() + delay);
    }

    fn clear_problem(&mut self) {
        if self.problem.take().is_some() {
            self.dirty = true;
        }
    }

    fn live(&self) -> Live {
        Live {
            side: self.config.side,
            remap: if self.config.swap_cmd_ctrl {
                KeyRemap::swap_command_control()
            } else {
                KeyRemap::none()
            },
        }
    }

    /// 连接没了：记下对方最后在线的时刻，清空与连接有关的状态。
    fn drop_link(&mut self) {
        if self.engine.is_none() {
            self.port = None;
        }
        if let Some(fingerprint) = self.peer_fingerprint.take() {
            self.last_seen.insert(fingerprint, now_ms());
        }
        self.peer = None;
        self.since = None;
        self.cursor_on_peer = false;
        self.rtt.clear();
        self.dirty = true;
    }

    fn peer_name(&self) -> String {
        self.peer
            .as_ref()
            .map_or_else(|| "对方".to_owned(), |p| p.name.clone())
    }

    // ── 引擎事件 ─────────────────────────────────

    pub(crate) fn on_engine(&mut self, event: Event) {
        let progress = matches!(
            &event,
            Event::Transfer(TransferUpdate {
                state: TransferState::Running,
                ..
            })
        );
        if !matches!(event, Event::Rtt(_)) && !progress {
            self.dirty = true;
        }
        match event {
            Event::Listening { port, announced } => {
                self.status = LinkStatus::Waiting;
                self.port = Some(port);
                self.clear_problem();
                let detail = if announced {
                    format!("等待另一台电脑连接 · 端口 {port}")
                } else {
                    format!("端口 {port} · 没有在局域网里广播")
                };
                self.log(LogKind::Link, LogIcon::Power, "已开始共享", detail);
            }
            Event::Searching => {
                if self.peer.is_none() {
                    self.status = LinkStatus::Searching;
                }
                self.clear_problem();
            }
            Event::Found(list) => self.found = list,
            Event::Connecting { .. } => {
                if self.peer.is_none() {
                    self.status = LinkStatus::Connecting;
                }
            }
            Event::Connected {
                name,
                fingerprint,
                addr,
            } => {
                self.status = LinkStatus::Connected;
                self.clear_problem();
                self.peer = Some(PeerRef {
                    name: name.clone(),
                    fingerprint: fingerprint.to_hex(),
                    short: fingerprint.short(),
                    addr: addr.ip().to_string(),
                });
                self.peer_fingerprint = Some(fingerprint);
                self.since = Some(now_ms());
                // 刚连上时光标总在服务端。
                self.cursor_on_peer = self.config.role == Role::Client;
                self.rtt.clear();
                self.refresh_peers();
                self.log(
                    LogKind::Link,
                    LogIcon::Link,
                    format!("已连接 {name}"),
                    format!("TLS 1.3 · {}", addr.ip()),
                );
            }
            Event::Disconnected { reason } => {
                let name = self.peer_name();
                self.drop_link();
                self.status = match self.config.role {
                    Role::Server => LinkStatus::Waiting,
                    Role::Client => LinkStatus::Searching,
                };
                self.log(
                    LogKind::Link,
                    LogIcon::Unlink,
                    format!("与 {name} 断开"),
                    reason.unwrap_or_else(|| "对方已断开".into()),
                );
            }
            Event::Rtt(rtt) => self.rtt.record(rtt),
            Event::Transfer(update) => self.on_transfer(update),
            Event::Cursor { here, hotkey } => {
                self.cursor_on_peer = !here;
                let peer = self.peer_name();
                let (text, detail) = match (self.config.role, here) {
                    (Role::Server, false) => {
                        self.activity.switched();
                        (
                            format!("光标切到 {peer}"),
                            format!("从本机{}边缘", side_label(self.config.side)),
                        )
                    }
                    (Role::Server, true) => (
                        "光标回到本机".to_owned(),
                        if hotkey {
                            "快捷键 Ctrl + Option + Esc"
                        } else {
                            "从对方屏幕边缘"
                        }
                        .to_owned(),
                    ),
                    (Role::Client, true) => {
                        self.activity.switched();
                        ("光标来到本机".to_owned(), format!("从 {peer}"))
                    }
                    (Role::Client, false) => (format!("光标回到 {peer}"), String::new()),
                };
                self.log(LogKind::Cursor, LogIcon::Cursor, text, detail);
            }
            Event::Clipboard { to_peer, summary } => {
                let peer = self.peer_name();
                self.activity.clipped();
                let detail = match summary.chars {
                    Some(n) => format!("文本 · {n} 字"),
                    None => format!("{} 字节", summary.bytes),
                };
                self.clipboard = Some(ClipboardSync {
                    to_peer,
                    peer: peer.clone(),
                    chars: summary.chars,
                    bytes: summary.bytes,
                    preview: summary.preview,
                    at: now_ms(),
                });
                let text = if to_peer {
                    format!("剪贴板同步到 {peer}")
                } else {
                    "剪贴板同步到本机".to_owned()
                };
                self.log(LogKind::Clip, LogIcon::Clipboard, text, detail);
            }
            Event::Paired { name, fingerprint } => {
                self.refresh_peers();
                self.log(
                    LogKind::Link,
                    LogIcon::Lock,
                    format!("已与 {name} 配对"),
                    format!("指纹 {}", fingerprint.short()),
                );
                self.toast(format!("已与 {name} 配对"));
            }
            Event::PairingRejected { name, by_peer } => {
                let text = if by_peer {
                    format!("{name} 拒绝了配对")
                } else {
                    format!("已拒绝 {name} 的配对请求")
                };
                self.log(LogKind::Link, LogIcon::Warning, text, "");
            }
        }
    }

    // ── 配对 ─────────────────────────────────────

    pub(crate) fn on_pairing(&mut self, ask: PairAsk) {
        let id = self.next_prompt;
        self.next_prompt += 1;
        let request = &ask.request;
        let prompt = PairingPrompt {
            id,
            peer_name: request.peer_name.clone(),
            fingerprint: request.peer_fingerprint.to_hex(),
            short: request.peer_fingerprint.short(),
            code: request.code.to_string(),
        };
        self.log(
            LogKind::Link,
            LogIcon::Lock,
            format!("{} 请求配对", request.peer_name),
            format!("配对码 {}", request.code),
        );
        self.broadcast(Notice::Pairing {
            prompt: prompt.clone(),
        });
        self.pending.insert(
            id,
            Pending {
                prompt,
                reply: ask.reply,
            },
        );
        // 没有界面在看：打开设置界面，用户才看得到配对码。
        if self.clients.is_empty() {
            self.host.open_settings();
        }
    }

    fn answer(&mut self, id: u64, accept: bool) {
        if let Some(pending) = self.pending.remove(&id) {
            let _ = pending.reply.send(accept);
            self.broadcast(Notice::PairingClosed { id });
        }
    }

    // ── 界面 ─────────────────────────────────────

    pub(crate) async fn on_ui(&mut self, event: UiEvent) {
        match event {
            UiEvent::Joined { id, outbox } => {
                let mut initial = vec![
                    Notice::History {
                        entries: self.log.entries(),
                    },
                    Notice::State {
                        state: Box::new(self.snapshot()),
                    },
                ];
                initial.extend(self.pending.values().map(|p| Notice::Pairing {
                    prompt: p.prompt.clone(),
                }));
                for notice in initial {
                    if outbox.try_send(notice).is_err() {
                        return;
                    }
                }
                self.clients.insert(id, outbox);
            }
            UiEvent::Request(request) => self.on_request(request).await,
            UiEvent::Left { id } => {
                self.clients.remove(&id);
            }
        }
    }

    async fn on_request(&mut self, request: Request) {
        match request {
            Request::Hello { .. } => {}
            Request::SetSharing { on } => self.set_sharing(on).await,
            Request::UpdateSettings { settings } => self.update_settings(settings).await,
            Request::Forget { fingerprint } => self.forget(&fingerprint).await,
            Request::PairReply { id, accept } => self.answer(id, accept),
            Request::Connect { fingerprint } => self.connect_to(&fingerprint).await,
            Request::OpenPermissionSettings => self.host.open_permission_settings(),
            Request::ClearLog => {
                self.log.clear();
                self.broadcast(Notice::History {
                    entries: Vec::new(),
                });
            }
            Request::OpenSettings => self.host.open_settings(),
            Request::SendFiles { paths } => self.send_files(paths).await,
        }
    }

    async fn send_files(&mut self, paths: Vec<String>) {
        let paths = paths.into_iter().map(PathBuf::from).collect();
        match self.files.send(paths).await {
            Ok(_) => {
                let peer = self.peer_name();
                self.toast(format!("开始发给 {peer}"));
            }
            Err(e) => self.toast(e.to_string()),
        }
    }

    /// 文件传输有了进展：开始、结束、中断都记进日志，过程中的进度不记。
    fn on_transfer(&mut self, update: TransferUpdate) {
        self.edge.transfer(update.clone());
        let peer = self.peer_name();
        let what = if update.items > 1 {
            format!("{} 等 {} 项", update.name, update.items)
        } else {
            update.name.clone()
        };
        let size = size_text(update.bytes);
        let files = format!("{} 个文件 · {size}", update.files);
        let (text, detail) = match (&update.state, update.outgoing) {
            (TransferState::Running, _) => return,
            (TransferState::Waiting, true) => (format!("发送 {what} 到 {peer}"), files),
            (TransferState::Waiting, false) => (format!("{peer} 发来 {what}"), files),
            (TransferState::Done { .. }, true) => {
                (format!("已发送 {what}"), format!("到 {peer} · {size}"))
            }
            (TransferState::Done { saved }, false) => {
                let place = saved
                    .first()
                    .and_then(|p| p.parent())
                    .map_or_else(String::new, |p| format!("保存在 {}", p.display()));
                (format!("已收到 {what}"), place)
            }
            (TransferState::Interrupted, _) => {
                (format!("传输中断：{what}"), "重新连上后接着传".into())
            }
            (TransferState::Cancelled { by_peer }, _) => (
                format!("已取消：{what}"),
                if *by_peer {
                    "对方取消了"
                } else {
                    "本机取消了"
                }
                .into(),
            ),
            (TransferState::Failed { reason }, _) => (format!("传输失败：{what}"), reason.clone()),
        };
        self.log(LogKind::File, LogIcon::File, text, detail);
    }

    pub(crate) async fn on_command(&mut self, command: Command) {
        match command {
            Command::SetSharing(on) => self.set_sharing(on).await,
            Command::OpenSettings => self.host.open_settings(),
            Command::Quit => {}
        }
    }

    async fn set_sharing(&mut self, on: bool) {
        if on != self.paused {
            return;
        }
        self.paused = !on;
        self.problem = None;
        self.dirty = true;
        if on {
            self.start_engine();
            self.log(LogKind::Link, LogIcon::Play, "继续共享", "键鼠可以切到对方");
            self.toast("已继续共享");
        } else {
            self.stop_engine().await;
            self.status = LinkStatus::Paused;
            self.log(LogKind::Link, LogIcon::Pause, "暂停共享", "键鼠留在本机");
            self.toast("已暂停共享");
        }
    }

    async fn update_settings(&mut self, patch: SettingsPatch) {
        let mut next = self.config.clone();
        // 需要重新握手才能生效的设置。
        let mut restart = false;
        if let Some(name) = patch.name {
            restart |= name != next.name;
            next.name = name;
        }
        if let Some(announce) = patch.announce {
            restart |= announce != next.announce;
            next.announce = announce;
        }
        if let Some(clipboard) = patch.clipboard {
            restart |= clipboard != next.clipboard;
            next.clipboard = clipboard;
        }
        if let Some(server) = patch.server {
            restart |= server != next.server;
            next.server = server;
        }
        let moved = patch.side.filter(|&side| side != next.side);
        if let Some(side) = patch.side {
            next.side = side;
        }
        if let Some(swap) = patch.swap_cmd_ctrl {
            next.swap_cmd_ctrl = swap;
        }
        if let Some(theme) = patch.theme {
            next.theme = theme;
        }
        if let Some(accent) = patch.accent {
            next.accent = accent;
            self.edge.set_accent(accent_color(accent));
        }
        if let Some(on) = patch.autostart {
            match self.host.set_autostart(on) {
                Ok(()) => self.autostart = self.host.autostart(),
                Err(e) => self.toast(format!("无法修改开机自启：{e}")),
            }
            self.dirty = true;
        }

        if next != self.config {
            if let Err(e) = self.store.save_config(&next) {
                self.toast(format!("保存设置失败：{e}"));
                return;
            }
            self.config = next;
            self.dirty = true;
        }
        if let Some(side) = moved {
            let peer = self.peer_name();
            self.log(
                LogKind::Link,
                LogIcon::Layout,
                format!("{peer} 移到本机{}", side_label(side)),
                "布局已保存",
            );
        }
        if restart {
            self.name = resolve_name(&self.config, &self.host);
            self.restart_engine().await;
        } else if let Some(live) = self.engine.as_ref().and_then(|t| t.live.as_ref()) {
            live.send_if_modified(|current| {
                let next = self.live();
                let changed = *current != next;
                *current = next;
                changed
            });
        }
    }

    async fn forget(&mut self, fingerprint: &str) {
        let Ok(fingerprint) = fingerprint.parse::<Fingerprint>() else {
            return;
        };
        let mut peers = match self.store.peers() {
            Ok(peers) => peers,
            Err(e) => return self.toast(format!("读取已配对设备失败：{e}")),
        };
        let Some(name) = peers.get(&fingerprint).map(|p| p.name.clone()) else {
            return;
        };
        peers.remove(&fingerprint);
        if let Err(e) = self.store.save_peers(&peers) {
            return self.toast(format!("移除失败：{e}"));
        }
        self.refresh_peers();
        self.log(
            LogKind::Link,
            LogIcon::Unlink,
            format!("已移除 {name}"),
            "下次连接需要重新配对",
        );
        self.toast(format!("已移除 {name}"));
        // 正连着这台：断开它，它下次来要重新配对。
        if self.peer_fingerprint == Some(fingerprint) {
            self.restart_engine().await;
        }
    }

    /// 客户端：连接局域网里发现的一台服务端。
    async fn connect_to(&mut self, fingerprint: &str) {
        if self.config.role != Role::Client {
            return;
        }
        let Ok(fingerprint) = fingerprint.parse::<Fingerprint>() else {
            return;
        };
        let Some(addr) = self
            .found
            .iter()
            .find(|f| f.fingerprint == fingerprint)
            .and_then(|f| f.addrs.first().copied())
        else {
            return self.toast("这台电脑已经不在局域网里了");
        };
        let patch = SettingsPatch {
            server: Some(addr.to_string()),
            ..SettingsPatch::default()
        };
        self.update_settings(patch).await;
    }

    // ── 定时 ─────────────────────────────────────

    /// 每 250 毫秒一次：结算每秒的统计、清理过期的配对请求、按时重试、推送状态。
    pub(crate) fn on_tick(&mut self) {
        let now = Instant::now();
        if now.duration_since(self.last_second) >= Duration::from_secs(1) {
            self.last_second = now;
            if self.rtt.roll() {
                self.dirty = true;
            }
            if self.peer.is_some() {
                self.activity.connected_for(1);
            }
        }

        // 引擎那边已经不等了（超时、对方拒绝、连接断开）的配对请求。
        let closed: Vec<u64> = self
            .pending
            .iter()
            .filter(|(_, p)| p.reply.is_closed())
            .map(|(&id, _)| id)
            .collect();
        for id in closed {
            self.pending.remove(&id);
            self.broadcast(Notice::PairingClosed { id });
        }

        if self.engine.is_none() && self.retry_at.is_some_and(|at| now >= at) {
            self.start_engine();
        }
        if now.duration_since(self.peers_read) >= PEERS_REFRESH {
            self.refresh_peers();
        }
        if self.dirty {
            self.dirty = false;
            if !self.clients.is_empty() {
                let state = Box::new(self.snapshot());
                self.broadcast(Notice::State { state });
            }
        }
    }

    fn refresh_peers(&mut self) {
        self.peers_read = Instant::now();
        match self.store.peers() {
            Ok(peers) if peers != self.peers => {
                self.peers = peers;
                self.dirty = true;
            }
            Ok(_) => {}
            Err(e) => warn!("读取已配对设备失败：{e}"),
        }
    }

    // ── 输出 ─────────────────────────────────────

    fn log(
        &mut self,
        kind: LogKind,
        icon: LogIcon,
        text: impl Into<String>,
        detail: impl Into<String>,
    ) {
        let entry = self.log.push(kind, icon, text, detail);
        info!(target: "screenferry", "{} {}", entry.text, entry.detail);
        self.broadcast(Notice::Log { entry });
    }

    fn toast(&mut self, text: impl Into<String>) {
        self.broadcast(Notice::Toast { text: text.into() });
    }

    /// 发给所有界面。某个界面积压太多就断开它，它重连后会重新同步。
    fn broadcast(&mut self, notice: Notice) {
        self.clients
            .retain(|id, outbox| match outbox.try_send(notice.clone()) {
                Ok(()) => true,
                Err(_) => {
                    warn!(id, "设置界面没有及时接收，已断开");
                    false
                }
            });
    }

    pub(crate) fn summary(&self) -> Summary {
        Summary {
            sharing: !self.paused,
            connected_to: self.peer.as_ref().map(|p| p.name.clone()),
            problem: self.problem.as_ref().map(|p| p.message.clone()),
        }
    }

    pub(crate) fn snapshot(&self) -> State {
        let online = self.peer_fingerprint;
        State {
            local: Local {
                name: self.name.clone(),
                fingerprint: self.fingerprint.to_hex(),
                short: self.fingerprint.short(),
                role: self.config.role,
                data_dir: self.store.root().display().to_string(),
                version: env!("CARGO_PKG_VERSION").to_owned(),
                port: self.port,
            },
            sharing: !self.paused,
            problem: self.problem.clone(),
            link: Link {
                status: self.status,
                peer: self.peer.clone(),
                since: self.since,
                cursor_on_peer: self.cursor_on_peer,
                rtt_ms: self.rtt.history(),
            },
            clipboard: self.clipboard.clone(),
            peers: self
                .peers
                .iter()
                .map(|p| PeerInfo {
                    name: p.name.clone(),
                    fingerprint: p.fingerprint.to_hex(),
                    short: p.fingerprint.short(),
                    online: online == Some(p.fingerprint),
                    last_seen: self.last_seen.get(&p.fingerprint).copied(),
                })
                .collect(),
            found: self
                .found
                .iter()
                .map(|f| FoundDevice {
                    name: f.name.clone(),
                    fingerprint: f.fingerprint.to_hex(),
                    short: f.fingerprint.short(),
                    addr: f
                        .addrs
                        .first()
                        .map(|a| a.ip().to_string())
                        .unwrap_or_default(),
                    paired: self.peers.contains(&f.fingerprint),
                })
                .collect(),
            settings: Settings {
                name: self.config.name.clone(),
                side: self.config.side,
                swap_cmd_ctrl: self.config.swap_cmd_ctrl,
                announce: self.config.announce,
                clipboard: self.config.clipboard,
                server: self.config.server.clone(),
                autostart: self.autostart,
                theme: self.config.theme,
                accent: self.config.accent,
            },
            activity: self.activity.recent(),
        }
    }
}

fn resolve_name<H: Host>(config: &Config, host: &H) -> String {
    match config.name.trim() {
        "" => host.device_name(),
        name => name.to_owned(),
    }
}

/// 解析 `IP`、`IP:端口`、`主机名` 或 `主机名:端口`，省略端口时用默认端口。
fn resolve(text: &str) -> Result<SocketAddr> {
    if let Ok(addr) = text.parse::<SocketAddr>() {
        return Ok(addr);
    }
    if let Ok(ip) = text.parse::<IpAddr>() {
        return Ok(SocketAddr::new(ip, DEFAULT_PORT));
    }
    let with_port = if text.contains(':') {
        text.to_owned()
    } else {
        format!("{text}:{DEFAULT_PORT}")
    };
    with_port
        .to_socket_addrs()
        .ok()
        .and_then(|mut addrs| addrs.next())
        .ok_or_else(|| Error::BadAddress(text.to_owned()))
}

fn side_label(side: Side) -> &'static str {
    match side {
        Side::Left => "左侧",
        Side::Right => "右侧",
        Side::Top => "上方",
        Side::Bottom => "下方",
    }
}

/// 文件大小，给人看的写法。
fn size_text(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if value < 10.0 {
        format!("{value:.1} {}", UNITS[unit])
    } else {
        format!("{value:.0} {}", UNITS[unit])
    }
}

/// 设置界面里第 `index` 个强调色（与 `app/lib/theme/tokens.dart` 中的浅色版一致）。
fn accent_color(index: u8) -> ferry_types::Rgba {
    const ACCENTS: [u32; 5] = [0x0A6CFF, 0x00796B, 0x6A4BD8, 0xC2410C, 0xC2185B];
    let rgb = ACCENTS
        .get(usize::from(index))
        .copied()
        .unwrap_or(ACCENTS[0]);
    ferry_types::Rgba::hex(rgb, 255)
}
