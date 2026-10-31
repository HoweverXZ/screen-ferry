//! 服务端：拥有键鼠的一方。捕获本机鼠标，光标越过边缘后把输入发给客户端。
//!
//! 任何原因结束会话（对方断开、心跳超时、出错）时，都会立即把控制权交还本机。

use std::convert::Infallible;

use ferry_core::{EdgeSwitch, Pressed, Side, Transition, bounds};
use ferry_net::Framed;
use ferry_platform::{CaptureEvent, Displays, InputCapture};
use ferry_proto::{Bye, ByeReason, Channel, Enter, Message};
use ferry_types::{Display, InputEvent, LockKeys, Point};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{self, UnboundedReceiver};
use tokio::time::{MissedTickBehavior, interval, timeout};
use tracing::{debug, info, warn};

use crate::error::{Error, Result};
use crate::heartbeat::{self, Heartbeat};
use crate::link::{CAPABILITIES, HANDSHAKE_TIMEOUT, primary_scale, send};

pub struct ServerConfig {
    /// 客户端的屏幕位于本机的哪一侧。
    pub side: Side,
    /// 本机设备名，显示给对方。
    pub name: String,
}

/// 运行服务端：在 `listener` 上一次服务一个客户端，断开后继续等待下一个。
///
/// 只有启动失败（例如捕获权限不足）时才返回错误。
pub async fn serve<C, D>(
    listener: TcpListener,
    config: ServerConfig,
    capture: C,
    displays: D,
) -> Result<Infallible>
where
    C: InputCapture,
    D: Displays,
{
    let mut server = Server::start(config, capture, displays)?;
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
        let result = server.session(stream, session).await;
        server.return_control();
        match result {
            Ok(()) => info!(%peer, "客户端已断开"),
            Err(e) => warn!(%peer, "连接中断：{e}"),
        }
    }
}

struct Server<C, D> {
    config: ServerConfig,
    capture: C,
    displays: D,
    events: UnboundedReceiver<CaptureEvent>,
    switch: Option<EdgeSwitch>,
    /// 本机按着的鼠标键。按着时不切换屏幕，避免拖拽到一半换了屏。
    buttons: Pressed,
}

impl<C: InputCapture, D: Displays> Server<C, D> {
    fn start(config: ServerConfig, mut capture: C, displays: D) -> Result<Self> {
        let (tx, events) = mpsc::unbounded_channel();
        capture.start(Box::new(move |event| {
            let _ = tx.send(event);
        }))?;
        Ok(Self {
            config,
            capture,
            displays,
            events,
            switch: None,
            buttons: Pressed::new(),
        })
    }

    async fn session(&mut self, stream: TcpStream, id: u64) -> Result<()> {
        let mut framed = ferry_net::wrap(stream, Channel::Input)?;
        let handshake = ferry_net::accept(&mut framed, CAPABILITIES, id, &self.config.name);
        let (hello, _) = timeout(HANDSHAKE_TIMEOUT, handshake)
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

        let mut heartbeat = Heartbeat::new();
        let mut tick = interval(heartbeat::TICK);
        tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            tokio::select! {
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

    async fn on_capture(
        &mut self,
        event: CaptureEvent,
        framed: &mut Framed<TcpStream>,
    ) -> Result<()> {
        let Some(switch) = self.switch.as_mut() else {
            return Ok(());
        };

        match event {
            CaptureEvent::Pointer { position, dx, dy } => {
                let free = !self.buttons.any_button();
                let transition = if switch.is_remote() {
                    switch.remote_motion(dx, dy, free)
                } else if free {
                    switch.local_motion(position, dx, dy)
                } else {
                    None
                };
                match transition {
                    Some(Transition::Enter { seq, at }) => {
                        // 先抓取再通知对方：抓取失败时，会话结束后会照常归还控制权。
                        self.capture.grab()?;
                        let enter = Enter {
                            seq,
                            position: at,
                            locks: LockKeys::empty(),
                        };
                        send(framed, &Message::Enter(enter)).await?;
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
                        // 先归还本机控制再通知对方：发送失败也不会把本机鼠标锁住。
                        self.capture.release(local_at)?;
                        send(framed, &Message::Leave { seq }).await?;
                        info!("光标回到本机");
                    }
                    None => {}
                }
            }
            CaptureEvent::Button { button, pressed } => {
                self.buttons.track(&InputEvent::Button {
                    button,
                    pressed,
                    position: Point::default(),
                });
                if let Some(position) = switch.remote_position() {
                    send(
                        framed,
                        &Message::Input(InputEvent::Button {
                            button,
                            pressed,
                            position,
                        }),
                    )
                    .await?;
                }
            }
            CaptureEvent::Wheel { dx, dy } => {
                if let Some(position) = switch.remote_position() {
                    send(
                        framed,
                        &Message::Input(InputEvent::Wheel { dx, dy, position }),
                    )
                    .await?;
                }
            }
            // 原型阶段键盘不转发。
            CaptureEvent::Key { .. } => {}
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
        if let Some(at) = self.switch.take().as_mut().and_then(EdgeSwitch::abort) {
            match self.capture.release(at) {
                Ok(()) => info!("已把控制权交还本机"),
                Err(e) => warn!("交还本机控制失败：{e}"),
            }
        }
    }
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
