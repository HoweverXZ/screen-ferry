//! 客户端：被控制的一方。连接服务端，把收到的输入注入本机。
//!
//! 光标离开、连接断开或心跳超时时，都会立即松开所有按着的键，杜绝卡键。

use std::convert::Infallible;
use std::net::SocketAddr;
use std::time::Duration;

use ferry_core::Pressed;
use ferry_platform::{Displays, Error as PlatformError, InputInject};
use ferry_proto::{Channel, Hello, Message};
use ferry_types::{InputEvent, LockKeys, Point};
use tokio::time::{sleep, timeout};
use tracing::{debug, info, warn};

use crate::error::{Error, Result};
use crate::link::{CAPABILITIES, CONNECT_TIMEOUT, HANDSHAKE_TIMEOUT, send};

/// 两次连接尝试之间的间隔。
const RETRY_DELAY: Duration = Duration::from_secs(1);
/// 超过这么久收不到任何消息（服务端至少每 0.5 秒发一次 Ping），视为断线。
const SILENCE_TIMEOUT: Duration = Duration::from_secs(2);

pub struct ClientConfig {
    pub server: SocketAddr,
    /// 本机设备名，显示给对方。
    pub name: String,
}

/// 运行客户端：连接服务端，断开后自动重连。
///
/// 只有启动失败（例如读不到本机显示器）时才返回错误。
pub async fn connect<I, D>(config: ClientConfig, inject: I, displays: D) -> Result<Infallible>
where
    I: InputInject,
    D: Displays,
{
    displays.displays()?;
    let mut client = Client {
        inject,
        pressed: Pressed::new(),
        cursor: Point::default(),
    };
    loop {
        info!(server = %config.server, "正在连接服务端");
        let result = client.session(&config, &displays).await;
        client.release_all();
        match result {
            Ok(()) => info!("连接已结束"),
            Err(e) => warn!("连接中断：{e}"),
        }
        sleep(RETRY_DELAY).await;
    }
}

struct Client<I> {
    inject: I,
    pressed: Pressed,
    cursor: Point,
}

impl<I: InputInject> Client<I> {
    async fn session<D: Displays>(&mut self, config: &ClientConfig, displays: &D) -> Result<()> {
        let mut framed = timeout(
            CONNECT_TIMEOUT,
            ferry_net::connect(config.server, Channel::Input),
        )
        .await
        .map_err(|_| Error::Timeout("连接"))??;
        let hello = Hello::new(Channel::Input, CAPABILITIES, 0, config.name.clone());
        let welcome = timeout(HANDSHAKE_TIMEOUT, ferry_net::initiate(&mut framed, hello))
            .await
            .map_err(|_| Error::Timeout("握手"))??;
        send(&mut framed, &Message::Displays(displays.displays()?)).await?;
        info!(server = %welcome.name, "已连接，等待光标进入");

        loop {
            let message = timeout(SILENCE_TIMEOUT, framed.recv())
                .await
                .map_err(|_| Error::HeartbeatTimeout)??;
            match message {
                None | Some(Message::Bye(_)) => return Ok(()),
                Some(Message::Ping { nonce }) => {
                    send(&mut framed, &Message::Pong { nonce }).await?
                }
                Some(Message::Enter(enter)) => {
                    self.move_to(enter.position);
                    // 只同步 Caps Lock：对方可能没有 Num Lock（例如 Mac），同步它会误关本机的小键盘。
                    if let Err(e) = self.inject.sync_locks(enter.locks, LockKeys::CAPS) {
                        warn!("同步锁定键失败：{e}");
                    }
                    info!(at = ?enter.position, "光标进入本机");
                }
                Some(Message::Leave { .. }) => {
                    self.release_all();
                    info!("光标离开本机");
                }
                Some(Message::Input(event)) => self.apply(event),
                Some(Message::Displays(_)) => {}
                Some(other) => debug!(?other, "忽略消息"),
            }
        }
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
