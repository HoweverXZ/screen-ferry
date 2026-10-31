//! 场景测试共用：在本机回环地址上跑一对真实的服务端与客户端，平台换成 mock。

use std::time::Duration;

use ferry_core::{KeyRemap, Side};
use ferry_engine::{ClientConfig, ServerConfig, connect, serve};
use ferry_platform::CaptureEvent;
use ferry_platform_mock::{MockCapture, MockDisplays, MockInject};
use ferry_types::{Display, DisplayId, KeyAction, KeyCode, Point, Rect};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio::time::sleep;

/// 服务端本机 1000×800（100%），客户端 1920×1080（150%），客户端在右侧。
/// 服务端在右边缘 y = 400 处推出去，进入客户端的 (0, 540)。
pub const EDGE: Point = Point::new(999, 400);
pub const ENTRY: Point = Point::new(0, 540);

pub struct Rig {
    pub capture: MockCapture,
    pub inject: MockInject,
    pub server: JoinHandle<()>,
    pub client: JoinHandle<()>,
}

fn display(width: u32, height: u32, scale_percent: u16) -> Display {
    Display {
        id: DisplayId(1),
        bounds: Rect::new(0, 0, width, height),
        scale_percent,
        primary: true,
    }
}

impl Rig {
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let capture = MockCapture::new();
        let inject = MockInject::new();

        let server = tokio::spawn({
            let capture = capture.clone();
            async move {
                let config = ServerConfig {
                    side: Side::Right,
                    name: "mac".into(),
                    remap: KeyRemap::swap_command_control(),
                };
                let displays = MockDisplays(vec![display(1000, 800, 100)]);
                serve(listener, config, capture, displays).await.unwrap();
            }
        });
        let client = tokio::spawn({
            let inject = inject.clone();
            async move {
                let config = ClientConfig {
                    server: addr,
                    name: "pc".into(),
                };
                let displays = MockDisplays(vec![display(1920, 1080, 150)]);
                connect(config, inject, displays).await.unwrap();
            }
        });
        Self {
            capture,
            inject,
            server,
            client,
        }
    }

    pub fn push(&self, dx: i32, dy: i32) {
        self.capture.emit(CaptureEvent::Pointer {
            position: EDGE,
            dx,
            dy,
        });
    }

    pub fn key(&self, code: KeyCode, action: KeyAction) {
        self.capture.emit(CaptureEvent::Key { code, action });
    }

    /// 在右边缘向外推，直到进入客户端屏幕。
    pub async fn enter(&self) {
        eventually("进入客户端屏幕", || {
            if self.capture.is_grabbed() {
                return true;
            }
            self.push(5, 0);
            false
        })
        .await;
    }
}

/// 反复检查，直到条件成立；3 秒内不成立就判定失败。
pub async fn eventually(what: &str, mut check: impl FnMut() -> bool) {
    for _ in 0..300 {
        if check() {
            return;
        }
        sleep(Duration::from_millis(10)).await;
    }
    panic!("等待超时：{what}");
}
