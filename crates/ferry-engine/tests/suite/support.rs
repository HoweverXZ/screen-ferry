//! 场景测试共用：在本机回环地址上跑一对真实的服务端与客户端，平台换成 mock。
//!
//! 连接是真实的 TLS，两端各有一个临时数据目录；配对确认由 [`AutoConfirm`] 自动回答。

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ferry_core::{KeyRemap, Side};
use ferry_engine::{
    ClientConfig, ConfirmPairing, EdgeFeed, Error, Events, Files, Live, PairingRequest, ServerAddr,
    ServerConfig, connect, serve,
};
use ferry_platform::CaptureEvent;
use ferry_platform_mock::{MockCapture, MockClipboard, MockDisplays, MockInject};
use ferry_store::Store;
use ferry_types::{Display, DisplayId, KeyAction, KeyCode, Point, Rect};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio::time::sleep;

/// 服务端本机 1000×800（100%），客户端 1920×1080（150%），客户端在右侧。
/// 服务端在右边缘 y = 400 处推出去一个单位，进入客户端的 (1, 540)：离边缘留出一个单位。
pub const EDGE: Point = Point::new(999, 400);
pub const ENTRY: Point = Point::new(1, 540);
/// 断开或按快捷键交还控制时，本机光标落在边缘往里一个单位。
pub const RETURN: Point = Point::new(998, 400);

/// 一台测试设备的数据目录。可以跨多个 `Rig` 复用，模拟重启后再连。
#[derive(Clone)]
pub struct Device {
    pub store: Store,
    /// 最后一个持有者丢弃时删除目录。
    _dir: Arc<TempDir>,
}

impl Device {
    pub fn new() -> Self {
        let dir = Arc::new(TempDir::new());
        let store = Store::open(dir.path()).unwrap();
        Self { store, _dir: dir }
    }
}

/// 自动回答配对确认，并记下每次的请求。
#[derive(Clone)]
pub struct AutoConfirm {
    answer: bool,
    requests: Arc<Mutex<Vec<PairingRequest>>>,
}

impl AutoConfirm {
    pub fn answering(answer: bool) -> Self {
        Self {
            answer,
            requests: Arc::default(),
        }
    }

    pub fn requests(&self) -> Vec<PairingRequest> {
        self.requests.lock().unwrap().clone()
    }
}

impl ConfirmPairing for AutoConfirm {
    async fn confirm(&self, request: &PairingRequest) -> bool {
        self.requests.lock().unwrap().push(request.clone());
        self.answer
    }
}

/// 启动参数。默认：两台全新设备、都同意配对、都启动。
pub struct Setup {
    pub mac: Device,
    pub pc: Device,
    pub mac_clipboard: MockClipboard,
    pub pc_clipboard: MockClipboard,
    pub mac_confirm: AutoConfirm,
    pub pc_confirm: AutoConfirm,
    pub start_client: bool,
    /// 服务端在局域网广播、客户端不填地址自己找。需要本机网络支持组播。
    pub discover: bool,
    /// 服务端可随时修改的设置；`None` 为固定的“右侧、互换 ⌘ 与 Ctrl”。
    pub live: Option<tokio::sync::watch::Receiver<Live>>,
    pub mac_events: Events,
    pub pc_events: Events,
    /// 服务端的边缘投递报告去处；默认不需要。
    pub edge: EdgeFeed,
    /// 服务端的文件传输；不给时按 `mac` 的数据目录新建。
    pub mac_files: Option<Files>,
    /// 客户端的边缘投递报告去处与文件传输；默认不需要、按 `pc` 的数据目录新建。
    pub pc_edge: EdgeFeed,
    pub pc_files: Option<Files>,
}

impl Default for Setup {
    fn default() -> Self {
        Self {
            mac: Device::new(),
            pc: Device::new(),
            mac_clipboard: MockClipboard::new(),
            pc_clipboard: MockClipboard::new(),
            mac_confirm: AutoConfirm::answering(true),
            pc_confirm: AutoConfirm::answering(true),
            start_client: true,
            discover: false,
            live: None,
            mac_events: Events::none(),
            pc_events: Events::none(),
            edge: EdgeFeed::none(),
            mac_files: None,
            pc_edge: EdgeFeed::none(),
            pc_files: None,
        }
    }
}

pub struct Rig {
    pub addr: SocketAddr,
    pub capture: MockCapture,
    pub inject: MockInject,
    /// 两端的显示器，可以中途修改。
    pub mac_displays: MockDisplays,
    pub pc_displays: MockDisplays,
    /// 两端的文件传输。收到的文件在各自数据目录的 `received/` 里。
    pub mac_files: Files,
    pub pc_files: Files,
    pc_events: Events,
    /// 服务端（Mac）。
    pub mac: Device,
    /// 客户端（PC）。
    pub pc: Device,
    pub mac_clipboard: MockClipboard,
    pub pc_clipboard: MockClipboard,
    pub mac_confirm: AutoConfirm,
    pub pc_confirm: AutoConfirm,
    /// 服务端与客户端只会因出错而结束，任务的结果就是那个错误。
    pub server: JoinHandle<Error>,
    pub client: JoinHandle<Error>,
}

pub fn display(width: u32, height: u32, scale_percent: u16) -> Display {
    Display {
        id: DisplayId(1),
        bounds: Rect::new(0, 0, width, height),
        scale_percent,
        primary: true,
    }
}

impl Rig {
    pub async fn start() -> Self {
        Self::with(Setup::default()).await
    }

    /// 用给定的剪贴板启动，便于测试连接之前就有内容的情况。
    pub async fn with_clipboards(
        mac_clipboard: MockClipboard,
        pc_clipboard: MockClipboard,
    ) -> Self {
        Self::with(Setup {
            mac_clipboard,
            pc_clipboard,
            ..Setup::default()
        })
        .await
    }

    pub async fn with(setup: Setup) -> Self {
        // 广播出去的是本机的局域网地址，所以发现模式要监听所有网卡。
        let bind = if setup.discover {
            "0.0.0.0:0"
        } else {
            "127.0.0.1:0"
        };
        let listener = TcpListener::bind(bind).await.unwrap();
        let addr = listener.local_addr().unwrap();
        let capture = MockCapture::new();
        let inject = MockInject::new();
        let mac_displays = MockDisplays::new(vec![display(1000, 800, 100)]);
        let pc_displays = MockDisplays::new(vec![display(1920, 1080, 150)]);
        let mac_files = setup
            .mac_files
            .clone()
            .unwrap_or_else(|| Files::new(&setup.mac.store, setup.mac_events.clone()));
        let pc_files = setup
            .pc_files
            .clone()
            .unwrap_or_else(|| Files::new(&setup.pc.store, setup.pc_events.clone()));

        let server = tokio::spawn({
            let capture = capture.clone();
            let displays = mac_displays.clone();
            let clipboard = setup.mac_clipboard.clone();
            let confirm = setup.mac_confirm.clone();
            let config = ServerConfig {
                live: setup.live.clone().unwrap_or_else(|| {
                    Live {
                        side: Side::Right,
                        remap: KeyRemap::swap_command_control(),
                    }
                    .fixed()
                }),
                name: "mac".into(),
                clipboard: true,
                store: setup.mac.store.clone(),
                announce: setup.discover,
                events: setup.mac_events.clone(),
                files: mac_files.clone(),
                edge: setup.edge.clone(),
            };
            async move {
                match serve(listener, config, capture, displays, clipboard, confirm).await {
                    Ok(never) => match never {},
                    Err(e) => e,
                }
            }
        });
        let client = if setup.start_client {
            Self::spawn_client(
                &setup,
                addr,
                inject.clone(),
                pc_displays.clone(),
                pc_files.clone(),
            )
        } else {
            tokio::spawn(std::future::pending())
        };
        Self {
            addr,
            capture,
            inject,
            mac_displays,
            pc_displays,
            mac_files,
            pc_files,
            pc_events: setup.pc_events.clone(),
            mac: setup.mac,
            pc: setup.pc,
            mac_clipboard: setup.mac_clipboard,
            pc_clipboard: setup.pc_clipboard,
            mac_confirm: setup.mac_confirm,
            pc_confirm: setup.pc_confirm,
            server,
            client,
        }
    }

    fn spawn_client(
        setup: &Setup,
        addr: SocketAddr,
        inject: MockInject,
        displays: MockDisplays,
        files: Files,
    ) -> JoinHandle<Error> {
        let clipboard = setup.pc_clipboard.clone();
        let confirm = setup.pc_confirm.clone();
        let config = ClientConfig {
            server: if setup.discover {
                ServerAddr::Discover
            } else {
                ServerAddr::Fixed(addr)
            },
            name: "pc".into(),
            clipboard: true,
            store: setup.pc.store.clone(),
            events: setup.pc_events.clone(),
            files,
            edge: setup.pc_edge.clone(),
        };
        tokio::spawn(async move {
            match connect(config, inject, displays, clipboard, confirm).await {
                Ok(never) => match never {},
                Err(e) => e,
            }
        })
    }

    /// 模拟断线：停掉客户端再重新启动，数据目录和文件传输都沿用原来的。
    pub fn restart_client(&mut self) {
        self.client.abort();
        let setup = Setup {
            pc: self.pc.clone(),
            pc_clipboard: self.pc_clipboard.clone(),
            pc_confirm: self.pc_confirm.clone(),
            pc_events: self.pc_events.clone(),
            ..Setup::default()
        };
        self.client = Self::spawn_client(
            &setup,
            self.addr,
            self.inject.clone(),
            self.pc_displays.clone(),
            self.pc_files.clone(),
        );
    }

    /// 停掉两端，留下数据目录供下一个 `Rig` 使用。
    pub fn stop(self) -> (Device, Device) {
        self.server.abort();
        self.client.abort();
        (self.mac, self.pc)
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

    /// 在右边缘向外推，直到进入客户端屏幕，并且客户端已处理 Enter（把光标移到入口）。
    pub async fn enter(&self) {
        let before = self.inject.events().len();
        eventually("进入客户端屏幕", || {
            if self.capture.is_grabbed() {
                return true;
            }
            self.push(1, 0);
            false
        })
        .await;
        eventually("客户端处理 Enter", || {
            self.inject.events().len() > before
        })
        .await;
    }

    /// 从客户端屏幕左边缘推回本机。
    pub async fn leave(&self) {
        eventually("回到本机", || {
            if !self.capture.is_grabbed() {
                return true;
            }
            self.push(-50, 0);
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

/// 测试结束时删除的临时目录。
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new() -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("ferry-engine-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
