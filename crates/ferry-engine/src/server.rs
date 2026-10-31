//! 服务端：拥有键鼠的一方。捕获本机键鼠，光标越过边缘后把输入发给客户端。
//!
//! 任何原因结束会话（对方断开、心跳超时、出错）时，都会立即把控制权交还本机。
//! 光标在对方屏幕时，按 Ctrl + Option（Alt）+ Esc 可以强制回到本机。
//!
//! 客户端握手后会用拿到的会话号再连一条数据连接，用来同步剪贴板。

use std::convert::Infallible;
use std::sync::{Arc, Mutex};

use ferry_core::{EdgeSwitch, KeyRemap, Pressed, Side, Transition, bounds};
use ferry_net::Framed;
use ferry_platform::{CaptureEvent, Clipboard, Displays, InputCapture};
use ferry_proto::{Bye, ByeReason, Capabilities, Channel, Enter, Message, RejectReason};
use ferry_types::{Display, InputEvent, KeyAction, KeyCode, Point};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{self, UnboundedReceiver};
use tokio::sync::watch;
use tokio::time::{MissedTickBehavior, interval, timeout};
use tracing::{debug, info, warn};

use crate::clipboard::{self, ClipboardTask, SharedClipboard};
use crate::error::{Error, Result};
use crate::heartbeat::{self, Heartbeat};
use crate::link::{HANDSHAKE_TIMEOUT, capabilities, primary_scale, send};

pub struct ServerConfig {
    /// 客户端的屏幕位于本机的哪一侧。
    pub side: Side,
    /// 本机设备名，显示给对方。
    pub name: String,
    /// 发送前对按键做的重映射，例如把 Command 当作对方的 Ctrl。
    pub remap: KeyRemap,
    /// 是否同步剪贴板。
    pub clipboard: bool,
}

/// 运行服务端：在 `listener` 上一次服务一个客户端，断开后继续等待下一个。
///
/// 只有启动失败（例如捕获权限不足）时才返回错误。
pub async fn serve<C, D, K>(
    listener: TcpListener,
    config: ServerConfig,
    capture: C,
    displays: D,
    clipboard: K,
) -> Result<Infallible>
where
    C: InputCapture,
    D: Displays,
    K: Clipboard,
{
    let mut server = Server::start(config, capture, displays, clipboard)?;
    info!(addr = %listener.local_addr()?, "等待客户端连接");

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
        info!(%peer, "客户端已连接");
        let result = server.session(stream, session, &listener).await;
        server.return_control();
        match result {
            Ok(()) => info!(%peer, "客户端已断开"),
            Err(e) => warn!(%peer, "连接中断：{e}"),
        }
    }
}

struct Server<C, D, K> {
    config: ServerConfig,
    capabilities: Capabilities,
    capture: C,
    displays: D,
    clipboard: SharedClipboard<K>,
    /// 光标是否在本机，供剪贴板同步判断谁是活动端。
    active: watch::Sender<bool>,
    events: UnboundedReceiver<CaptureEvent>,
    switch: Option<EdgeSwitch>,
    /// 按着的鼠标键和（光标在对方屏幕时）按着的键。
    /// 按着鼠标键时不切换屏幕，避免拖拽到一半换了屏；按着的键用来识别快捷键。
    pressed: Pressed,
}

impl<C: InputCapture, D: Displays, K: Clipboard> Server<C, D, K> {
    fn start(config: ServerConfig, mut capture: C, displays: D, clipboard: K) -> Result<Self> {
        let (tx, events) = mpsc::unbounded_channel();
        capture.start(Box::new(move |event| {
            let _ = tx.send(event);
        }))?;
        Ok(Self {
            capabilities: capabilities(config.clipboard),
            config,
            capture,
            displays,
            clipboard: Arc::new(Mutex::new(clipboard)),
            active: watch::Sender::new(true),
            events,
            switch: None,
            pressed: Pressed::new(),
        })
    }

    async fn session(&mut self, stream: TcpStream, id: u64, listener: &TcpListener) -> Result<()> {
        let mut framed = ferry_net::wrap(stream, Channel::Input)?;
        let handshake =
            ferry_net::accept(&mut framed, self.capabilities, &self.config.name, |hello| {
                match hello.channel {
                    Channel::Input => Ok(id),
                    // 数据连接只能在输入连接之后、带着它的会话号来。
                    Channel::Data => Err(RejectReason::UNKNOWN_SESSION),
                }
            });
        let (hello, welcome) = timeout(HANDSHAKE_TIMEOUT, handshake)
            .await
            .map_err(|_| Error::Timeout("握手"))??;

        let local = self.displays.displays()?;
        send(&mut framed, &Message::Displays(local.clone())).await?;
        let remote = timeout(HANDSHAKE_TIMEOUT, expect_displays(&mut framed))
            .await
            .map_err(|_| Error::Timeout("等待对方的显示器信息"))??;

        let local_rect = bounds(&local).ok_or(Error::NoDisplays("本机"))?;
        let remote_rect = bounds(&remote).ok_or(Error::NoDisplays("对方"))?;
        let scale = relative_scale(&local, &remote);
        info!(
            peer = %hello.name,
            local = ?local_rect,
            remote = ?remote_rect,
            scale,
            "握手完成：把光标推出本机 {:?} 侧边缘即可切换过去",
            self.config.side,
        );
        self.switch = Some(EdgeSwitch::new(
            local_rect,
            remote_rect,
            self.config.side,
            scale,
        ));

        self.active.send_replace(true);
        let clipboard = welcome.capabilities.contains(Capabilities::CLIPBOARD);
        let (data_tx, mut data_rx) = mpsc::channel(1);
        // 会话结束时随之丢弃，剪贴板同步也就停了。
        let mut _clipboard_task: Option<ClipboardTask> = None;

        let mut heartbeat = Heartbeat::new();
        let mut tick = interval(heartbeat::TICK);
        tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                accepted = listener.accept() => match accepted {
                    Ok((stream, peer)) => self.accept_data(stream, id, clipboard, data_tx.clone(), peer),
                    Err(e) => warn!("接受连接失败：{e}"),
                },
                Some(data) = data_rx.recv() => {
                    info!("数据连接已建立，开始同步剪贴板");
                    _clipboard_task = Some(clipboard::spawn(data, self.clipboard.clone(), self.active.subscribe()));
                }
                event = self.events.recv() => {
                    let Some(event) = event else { return Ok(()) };
                    self.on_capture(event, &mut framed).await?;
                }
                message = framed.recv() => match message? {
                    Some(Message::Pong { nonce }) => heartbeat.pong(nonce),
                    Some(Message::Ping { nonce }) => send(&mut framed, &Message::Pong { nonce }).await?,
                    Some(Message::Displays(displays)) => self.update_remote(&local, &displays),
                    Some(Message::Bye(_)) | None => return Ok(()),
                    Some(other) => debug!(?other, "忽略消息"),
                },
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
    fn accept_data(
        &self,
        stream: TcpStream,
        id: u64,
        clipboard: bool,
        established: mpsc::Sender<Framed<TcpStream>>,
        peer: std::net::SocketAddr,
    ) {
        let capabilities = self.capabilities;
        let name = self.config.name.clone();
        tokio::spawn(async move {
            let handshake = async {
                let mut framed = ferry_net::wrap(stream, Channel::Data)?;
                ferry_net::accept(&mut framed, capabilities, &name, |hello| {
                    match hello.channel {
                        Channel::Data if hello.session == id && clipboard => Ok(id),
                        Channel::Data => Err(RejectReason::UNKNOWN_SESSION),
                        Channel::Input => Err(RejectReason::BUSY),
                    }
                })
                .await?;
                Ok::<_, Error>(framed)
            };
            match timeout(HANDSHAKE_TIMEOUT, handshake).await {
                Ok(Ok(framed)) => {
                    let _ = established.send(framed).await;
                }
                Ok(Err(e)) => warn!(%peer, "拒绝了一个连接：{e}"),
                Err(_) => warn!(%peer, "新连接握手超时"),
            }
        });
    }

    async fn on_capture(
        &mut self,
        event: CaptureEvent,
        framed: &mut Framed<TcpStream>,
    ) -> Result<()> {
        let Some(switch) = self.switch.as_mut() else {
            return Ok(());
        };

        let transition = match event {
            CaptureEvent::Pointer { position, dx, dy } => {
                let free = !self.pressed.any_button();
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
                if let Some(position) = switch.remote_position() {
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
                if let Some(position) = switch.remote_position() {
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
                    switch.leave()
                } else {
                    let code = self.config.remap.apply(code);
                    send(framed, &Message::Input(InputEvent::Key { code, action })).await?;
                    None
                }
            }
        };

        match transition {
            Some(Transition::Enter { seq, at }) => {
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
                // 对方收到 Leave 会松开所有键，这些键不再需要跟踪。
                self.pressed.clear_keys();
                self.active.send_replace(true);
                send(framed, &Message::Leave { seq }).await?;
                info!("光标回到本机");
            }
            None => {}
        }
        Ok(())
    }

    fn is_remote(&self) -> bool {
        self.switch.as_ref().is_some_and(EdgeSwitch::is_remote)
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

/// Ctrl + Option（Alt）是否都按着。左右两侧的键都算。
fn escape_hotkey(pressed: &Pressed) -> bool {
    let held = |a, b| pressed.is_held(a) || pressed.is_held(b);
    held(KeyCode::LEFT_CTRL, KeyCode::RIGHT_CTRL) && held(KeyCode::LEFT_ALT, KeyCode::RIGHT_ALT)
}

async fn expect_displays(framed: &mut Framed<TcpStream>) -> Result<Vec<Display>> {
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
