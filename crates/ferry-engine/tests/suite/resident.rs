//! 常驻运行：用真实的本机通信当“设置界面”，平台换成 mock。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

use ferry_engine::resident::{self, Command, Host};
use ferry_engine::{ClientConfig, EdgeFeed, Events, Files, ServerAddr, connect};
use ferry_ipc::{
    Connection, Endpoint, LinkStatus, Notice, PairingPrompt, ProblemKind, Request, SettingsPatch,
    State,
};
use ferry_platform::Error as PlatformError;
use ferry_platform_mock::{MockCapture, MockClipboard, MockDisplays, MockInject, MockOverlay};
use ferry_types::{Config, Display, DisplayId, Rect, Role, Side, Theme};
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout};

use crate::support::{AutoConfirm, Device};

#[derive(Clone, Default)]
struct MockHost {
    /// 服务端监听的端口；默认 0，由系统分配，实际端口从状态里的 `local.port` 读。
    port: u16,
    deny_permissions: Arc<AtomicBool>,
    autostart: Arc<AtomicBool>,
    settings_opened: Arc<AtomicU32>,
    capture: MockCapture,
}

impl Host for MockHost {
    type Capture = MockCapture;
    type Inject = MockInject;
    type Displays = MockDisplays;
    type Clipboard = MockClipboard;
    type Overlay = MockOverlay;

    fn device_name(&self) -> String {
        "测试 Mac".into()
    }
    fn check_permissions(&self, _role: Role) -> ferry_platform::Result<()> {
        if self.deny_permissions.load(Ordering::SeqCst) {
            Err(PlatformError::PermissionDenied("测试中拒绝"))
        } else {
            Ok(())
        }
    }
    fn capture(&self) -> ferry_platform::Result<MockCapture> {
        Ok(self.capture.clone())
    }
    fn inject(&self) -> ferry_platform::Result<MockInject> {
        Ok(MockInject::new())
    }
    fn displays(&self) -> MockDisplays {
        MockDisplays::new(vec![Display {
            id: DisplayId(1),
            bounds: Rect::new(0, 0, 1000, 800),
            scale_percent: 100,
            primary: true,
        }])
    }
    fn clipboard(&self) -> MockClipboard {
        MockClipboard::new()
    }

    fn overlay(&self) -> MockOverlay {
        MockOverlay::new()
    }
    fn autostart(&self) -> bool {
        self.autostart.load(Ordering::SeqCst)
    }
    fn set_autostart(&self, on: bool) -> ferry_platform::Result<()> {
        self.autostart.store(on, Ordering::SeqCst);
        Ok(())
    }
    fn open_settings(&self) {
        self.settings_opened.fetch_add(1, Ordering::SeqCst);
    }
    fn open_permission_settings(&self) {}
    fn listen_port(&self) -> u16 {
        self.port
    }
}

/// 一份常驻后台，加上连着它的“设置界面”。
struct Daemon {
    device: Device,
    host: MockHost,
    handle: resident::Handle,
    task: JoinHandle<ferry_engine::Result<()>>,
}

impl Daemon {
    async fn start(host: MockHost) -> Self {
        let device = Device::new();
        device
            .store
            .save_config(&Config {
                role: Role::Server,
                announce: false,
                ..Config::default()
            })
            .unwrap();
        let (control, handle) = resident::control();
        let task = tokio::spawn(resident::run(device.store.clone(), host.clone(), control));
        let daemon = Self {
            device,
            host,
            handle,
            task,
        };
        daemon.endpoint().await;
        daemon
    }

    async fn endpoint(&self) -> Endpoint {
        let path = self.device.store.ipc_endpoint_path();
        for _ in 0..300 {
            if let Ok(Some(endpoint)) = Endpoint::load(&path) {
                return endpoint;
            }
            sleep(Duration::from_millis(10)).await;
        }
        panic!("后台没有写下 ipc.json");
    }

    async fn ui(&self) -> Ui {
        let conn = ferry_ipc::connect(&self.endpoint().await).await.unwrap();
        Ui { conn, state: None }
    }
}

/// 测试里的设置界面：记住最新的状态。
struct Ui {
    conn: Connection,
    state: Option<State>,
}

impl Ui {
    async fn send(&mut self, request: Request) {
        self.conn.send(&request).await.unwrap();
    }

    /// 读通知，直到 `check` 返回 `Some`。3 秒内等不到就失败。
    async fn wait<T>(&mut self, what: &str, mut check: impl FnMut(&Notice) -> Option<T>) -> T {
        let read = async {
            loop {
                let notice: Notice = self.conn.recv().await.unwrap().expect("后台断开了");
                if let Notice::State { state } = &notice {
                    self.state = Some((**state).clone());
                }
                if let Some(found) = check(&notice) {
                    return found;
                }
            }
        };
        timeout(Duration::from_secs(5), read)
            .await
            .unwrap_or_else(|_| panic!("等待超时：{what}"))
    }

    async fn state_where(&mut self, what: &str, pred: impl Fn(&State) -> bool) -> State {
        if let Some(state) = self.state.as_ref().filter(|s| pred(s)) {
            return state.clone();
        }
        self.wait(what, |n| match n {
            Notice::State { state } if pred(state) => Some((**state).clone()),
            _ => None,
        })
        .await
    }
}

#[tokio::test]
async fn the_ui_sees_the_state_and_can_pause_sharing() {
    let daemon = Daemon::start(MockHost::default()).await;
    let mut ui = daemon.ui().await;

    let state = ui
        .state_where("开始等待连接", |s| {
            s.link.status == LinkStatus::Waiting
        })
        .await;
    assert_eq!(state.local.name, "测试 Mac");
    assert_eq!(state.local.role, Role::Server);
    assert!(state.sharing);
    assert_eq!(state.local.fingerprint.len(), 64);

    ui.send(Request::SetSharing { on: false }).await;
    ui.wait("暂停的提示", |n| match n {
        Notice::Toast { text } if text == "已暂停共享" => Some(()),
        _ => None,
    })
    .await;
    ui.state_where("已暂停", |s| {
        !s.sharing && s.link.status == LinkStatus::Paused
    })
    .await;

    ui.send(Request::SetSharing { on: true }).await;
    ui.state_where("继续等待连接", |s| {
        s.sharing && s.link.status == LinkStatus::Waiting
    })
    .await;
    daemon.task.abort();
}

#[tokio::test]
async fn settings_are_saved_and_reported_back() {
    let daemon = Daemon::start(MockHost::default()).await;
    let mut ui = daemon.ui().await;
    ui.send(Request::UpdateSettings {
        settings: SettingsPatch {
            side: Some(Side::Left),
            theme: Some(Theme::Dark),
            accent: Some(3),
            autostart: Some(true),
            ..SettingsPatch::default()
        },
    })
    .await;
    let state = ui
        .state_where("设置已生效", |s| s.settings.side == Side::Left)
        .await;
    assert_eq!(state.settings.theme, Theme::Dark);
    assert_eq!(state.settings.accent, 3);
    assert!(state.settings.autostart, "开机自启以系统中的登记为准");

    let saved = daemon.device.store.config().unwrap();
    assert_eq!(saved.side, Side::Left);
    assert_eq!(saved.theme, Theme::Dark);
    daemon.task.abort();
}

#[tokio::test]
async fn device_settings_are_saved_and_reported_back() {
    use ferry_types::DeviceSettings;

    let daemon = Daemon::start(MockHost::default()).await;
    let mut ui = daemon.ui().await;
    let fingerprint = "AB".repeat(32);
    let mut device = DeviceSettings::new(fingerprint.clone());
    device.alias = "书房的电脑".into();
    device.reverse_vertical = true;
    device.pointer_dpi = 50_000;
    ui.send(Request::SetDevice { device }).await;
    ui.send(Request::UpdateSettings {
        settings: SettingsPatch {
            mouse_dpi: Some(800),
            ..SettingsPatch::default()
        },
    })
    .await;
    let state = ui
        .state_where("设备设置已生效", |s| {
            !s.settings.devices.is_empty() && s.settings.mouse_dpi == 800
        })
        .await;
    let saved = &state.settings.devices[0];
    assert_eq!(
        saved.fingerprint,
        fingerprint.to_lowercase(),
        "统一写成小写"
    );
    assert_eq!(saved.alias, "书房的电脑");
    assert!(saved.reverse_vertical);
    assert_eq!(
        saved.pointer_dpi,
        *DeviceSettings::POINTER_DPI.end(),
        "超出范围的收进范围内"
    );

    let config = daemon.device.store.config().unwrap();
    assert_eq!(config.devices, state.settings.devices);
    assert_eq!(config.mouse_dpi, 800);

    // 恢复默认：这一条从配置里去掉。
    ui.send(Request::SetDevice {
        device: DeviceSettings::new(fingerprint),
    })
    .await;
    ui.state_where("恢复默认", |s| s.settings.devices.is_empty())
        .await;
    assert!(daemon.device.store.config().unwrap().devices.is_empty());
    daemon.task.abort();
}

#[tokio::test]
async fn first_pairing_is_confirmed_in_the_ui() {
    let daemon = Daemon::start(MockHost::default()).await;
    let mut ui = daemon.ui().await;
    let state = ui
        .state_where("开始等待连接", |s| {
            s.link.status == LinkStatus::Waiting
        })
        .await;
    let port = state.local.port.expect("等待连接时报告监听的端口");

    // 一台全新的客户端连过来，需要配对。
    let pc = Device::new();
    let pc_confirm = AutoConfirm::answering(true);
    let client = tokio::spawn(connect(
        ClientConfig {
            server: ServerAddr::Fixed(([127, 0, 0, 1], port).into()),
            name: "pc".into(),
            clipboard: true,

            clipboard_limit: Default::default(),
            command_key: false,
            store: pc.store.clone(),
            events: Events::none(),
            files: Files::new(&pc.store, Events::none()),
            edge: EdgeFeed::none(),
            devices: Vec::new(),
            commands: None,
        },
        MockInject::new(),
        MockDisplays::new(vec![Display {
            id: DisplayId(1),
            bounds: Rect::new(0, 0, 1920, 1080),
            scale_percent: 150,
            primary: true,
        }]),
        MockClipboard::new(),
        pc_confirm.clone(),
    ));

    let prompt: PairingPrompt = ui
        .wait("配对请求", |n| match n {
            Notice::Pairing { prompt } => Some(prompt.clone()),
            _ => None,
        })
        .await;
    assert_eq!(prompt.peer_name, "pc");
    assert_eq!(prompt.code.len(), 7, "形如 123 456：{}", prompt.code);
    ui.send(Request::PairReply {
        id: prompt.id,
        accept: true,
    })
    .await;
    ui.wait("配对对话框关闭", |n| match n {
        Notice::PairingClosed { id } if *id == prompt.id => Some(()),
        _ => None,
    })
    .await;

    let state = ui
        .state_where("连上了", |s| s.link.status == LinkStatus::Connected)
        .await;
    let peer = state.link.peer.expect("连上时有对方");
    assert_eq!(peer.name, "pc");
    assert_eq!(peer.addr, "127.0.0.1");
    // 两边显示同一个配对码。
    assert_eq!(pc_confirm.requests()[0].code.to_string(), prompt.code);
    let state = ui
        .state_where("已配对列表里有它且在线", |s| {
            s.peers.iter().any(|p| p.name == "pc" && p.online)
        })
        .await;

    // 从界面移除它：断开，并从信任列表删掉。
    let fingerprint = state.peers[0].fingerprint.clone();
    ui.send(Request::Forget { fingerprint }).await;
    ui.state_where("移除后列表为空", |s| s.peers.is_empty())
        .await;
    assert!(daemon.device.store.peers().unwrap().is_empty());

    client.abort();
    daemon.task.abort();
}

#[tokio::test]
async fn missing_permission_is_reported_and_retried() {
    let host = MockHost {
        deny_permissions: Arc::new(AtomicBool::new(true)),
        ..MockHost::default()
    };
    let daemon = Daemon::start(host.clone()).await;
    let mut ui = daemon.ui().await;
    let state = ui
        .state_where("报告缺少权限", |s| s.problem.is_some())
        .await;
    assert_eq!(state.problem.unwrap().kind, ProblemKind::Permission);
    assert_eq!(state.link.status, LinkStatus::Stopped);

    // 用户去系统设置里授权之后，几秒内自动恢复。
    host.deny_permissions.store(false, Ordering::SeqCst);
    ui.state_where("授权后开始等待连接", |s| {
        s.problem.is_none() && s.link.status == LinkStatus::Waiting
    })
    .await;
    daemon.task.abort();
}

#[tokio::test]
async fn a_second_launch_hands_over_to_the_running_one() {
    let daemon = Daemon::start(MockHost::default()).await;
    assert!(resident::hand_over_to_running(&daemon.device.store).await);
    let opened = daemon.host.settings_opened.clone();
    crate::support::eventually("请运行中的那份打开设置界面", || {
        opened.load(Ordering::SeqCst) == 1
    })
    .await;

    // 没有在运行的后台时，什么也不做。
    let other = Device::new();
    assert!(!resident::hand_over_to_running(&other.store).await);
    daemon.task.abort();
}

#[tokio::test]
async fn quitting_removes_the_endpoint_file() {
    let daemon = Daemon::start(MockHost::default()).await;
    let path = daemon.device.store.ipc_endpoint_path();
    assert!(path.exists());
    daemon.handle.send(Command::Quit);
    let result = timeout(Duration::from_secs(3), daemon.task).await.unwrap();
    assert!(result.unwrap().is_ok());
    assert!(!path.exists(), "退出后界面应当知道后台已经不在");
}

#[tokio::test]
async fn pairing_with_no_ui_open_opens_the_settings() {
    let host = MockHost::default();
    let daemon = Daemon::start(host.clone()).await;
    // 先用界面看一眼监听的端口，再关掉界面。
    let port = {
        let mut ui = daemon.ui().await;
        let state = ui
            .state_where("开始等待连接", |s| {
                s.link.status == LinkStatus::Waiting
            })
            .await;
        state.local.port.expect("等待连接时报告监听的端口")
    };
    // 等后台处理完界面断开，否则它会以为还有界面在看。
    sleep(Duration::from_millis(200)).await;
    let pc = Device::new();
    let client = tokio::spawn(connect(
        ClientConfig {
            server: ServerAddr::Fixed(([127, 0, 0, 1], port).into()),
            name: "pc".into(),
            clipboard: false,

            clipboard_limit: Default::default(),
            command_key: false,
            store: pc.store.clone(),
            events: Events::none(),
            files: Files::new(&pc.store, Events::none()),
            edge: EdgeFeed::none(),
            devices: Vec::new(),
            commands: None,
        },
        MockInject::new(),
        MockDisplays::default(),
        MockClipboard::new(),
        AutoConfirm::answering(true),
    ));
    let opened = host.settings_opened.clone();
    crate::support::eventually("没有界面时打开设置界面", || {
        opened.load(Ordering::SeqCst) >= 1
    })
    .await;

    // 稍后打开的界面会收到仍在等待的配对请求。
    let mut ui = daemon.ui().await;
    ui.wait("补发配对请求", |n| match n {
        Notice::Pairing { .. } => Some(()),
        _ => None,
    })
    .await;
    client.abort();
    daemon.task.abort();
}

#[tokio::test]
async fn the_listening_port_is_reported_and_cleared_when_paused() {
    let daemon = Daemon::start(MockHost::default()).await;
    let mut ui = daemon.ui().await;
    let state = ui
        .state_where("开始等待连接", |s| {
            s.link.status == LinkStatus::Waiting
        })
        .await;
    assert!(
        state.local.port.is_some_and(|p| p != 0),
        "报告系统分配的实际端口"
    );

    ui.send(Request::SetSharing { on: false }).await;
    ui.state_where("暂停后不再监听", |s| s.local.port.is_none())
        .await;
    daemon.task.abort();
}

#[tokio::test]
async fn switching_the_role_restarts_as_the_other_side() {
    let daemon = Daemon::start(MockHost::default()).await;
    let mut ui = daemon.ui().await;
    ui.state_where("先以服务端等待连接", |s| {
        s.link.status == LinkStatus::Waiting && s.local.port.is_some()
    })
    .await;

    ui.send(Request::UpdateSettings {
        settings: SettingsPatch {
            role: Some(Role::Client),
            ..SettingsPatch::default()
        },
    })
    .await;
    let state = ui
        .state_where("改为客户端，不再监听", |s| {
            s.local.role == Role::Client && s.local.port.is_none()
        })
        .await;
    assert_ne!(state.link.status, LinkStatus::Waiting, "不再等别人来连");
    assert_eq!(state.local.platform, std::env::consts::OS);
    assert_eq!(daemon.device.store.config().unwrap().role, Role::Client);

    ui.send(Request::UpdateSettings {
        settings: SettingsPatch {
            role: Some(Role::Server),
            ..SettingsPatch::default()
        },
    })
    .await;
    ui.state_where("改回服务端后重新监听", |s| {
        s.local.role == Role::Server && s.local.port.is_some()
    })
    .await;
    daemon.task.abort();
}
