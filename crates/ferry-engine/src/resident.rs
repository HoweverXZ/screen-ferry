//! 常驻运行：按 `config.toml` 启动引擎，经本机通信把状态推给设置界面、执行界面的命令。
//!
//! 暂停共享即停止引擎；改了需要重新握手的设置（设备名、剪贴板、广播、服务端地址）就重启引擎；
//! 屏幕方向和 ⌘/Ctrl 互换经 [`Live`](crate::Live) 即时生效。首次配对的配对码交给设置界面核对，
//! 没有界面连着时先打开它。见 ADR 0004。

mod confirm;
mod record;
mod runtime;
mod ui;

use std::path::PathBuf;
use std::time::Duration;

use ferry_ipc::{Endpoint, Listener};
use ferry_platform::{Clipboard, Displays, InputCapture, InputInject};
use ferry_store::Store;
use ferry_types::Role;
use tokio::sync::{mpsc, watch};
use tokio::time::{MissedTickBehavior, interval};
use tracing::{info, warn};

use crate::error::Result;
use crate::events::Events;
use runtime::Runtime;

/// 状态推送与各种计时的节拍。界面最多每秒收到 4 次状态。
const TICK: Duration = Duration::from_millis(250);

/// 由应用提供的本机能力：按需创建平台对象，以及只有应用才知道怎么做的事。
pub trait Host: Send + Sync + 'static {
    type Capture: InputCapture + 'static;
    type Inject: InputInject + 'static;
    type Displays: Displays + Sync + 'static;
    type Clipboard: Clipboard;

    /// 本机缺省的设备名（计算机名）。
    fn device_name(&self) -> String;
    /// 检查本机作为 `role` 所需的权限。
    fn check_permissions(&self, role: Role) -> ferry_platform::Result<()>;
    fn capture(&self) -> ferry_platform::Result<Self::Capture>;
    fn inject(&self) -> ferry_platform::Result<Self::Inject>;
    fn displays(&self) -> Self::Displays;
    fn clipboard(&self) -> Self::Clipboard;
    /// 是否已登记为登录时启动。
    fn autostart(&self) -> bool;
    fn set_autostart(&self, on: bool) -> ferry_platform::Result<()>;
    /// 打开（或前置）设置界面。
    fn open_settings(&self);
    /// 打开系统中授予键鼠控制权限的页面。
    fn open_permission_settings(&self);
    /// 服务端监听的端口。
    fn listen_port(&self) -> u16 {
        crate::DEFAULT_PORT
    }
}

/// 托盘菜单等应用自身发来的命令。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    SetSharing(bool),
    OpenSettings,
    /// 停止引擎、删除 `ipc.json`，然后 [`run`] 返回。
    Quit,
}

/// 给托盘看的简要状态。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub sharing: bool,
    /// 已连接时是对方的设备名。
    pub connected_to: Option<String>,
    /// 妨碍工作的问题。
    pub problem: Option<String>,
}

/// [`run`] 一侧：接收命令、发布简要状态。
pub struct Control {
    commands: mpsc::UnboundedReceiver<Command>,
    summary: watch::Sender<Summary>,
}

/// 应用一侧：发命令、看简要状态。可以克隆，例如托盘菜单的回调各持一份。
#[derive(Clone)]
pub struct Handle {
    commands: mpsc::UnboundedSender<Command>,
    summary: watch::Receiver<Summary>,
}

impl Handle {
    pub fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }

    pub fn summary(&self) -> watch::Receiver<Summary> {
        self.summary.clone()
    }
}

/// 新建一对控制端。
pub fn control() -> (Control, Handle) {
    let (commands_tx, commands) = mpsc::unbounded_channel();
    let (summary_tx, summary) = watch::channel(Summary::default());
    (
        Control {
            commands,
            summary: summary_tx,
        },
        Handle {
            commands: commands_tx,
            summary,
        },
    )
}

/// 常驻运行，直到收到 [`Command::Quit`] 或 [`Handle`] 全部被丢弃。
///
/// 启动时在 `store` 指定的位置写下 `ipc.json`，退出时删除。
/// 只有无法启动（例如数据目录不可用）时返回错误；引擎出的问题报告给界面，过一会儿自动重试。
pub async fn run<H: Host>(store: Store, host: H, mut control: Control) -> Result<()> {
    let (listener, endpoint) = Listener::bind().await?;
    let endpoint_path = store.ipc_endpoint_path();
    endpoint.save(&endpoint_path)?;
    let _endpoint = RemoveOnDrop {
        path: endpoint_path,
        pid: endpoint.pid,
    };
    info!(port = endpoint.port, "设置界面可以连接了");

    let (events, mut engine_events) = Events::channel();
    let (asks, mut pair_asks) = mpsc::unbounded_channel();
    let (ui_tx, mut ui_events) = mpsc::unbounded_channel();
    let mut rt = Runtime::new(store, host, events, asks)?;
    rt.start_engine();

    let mut tick = interval(TICK);
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            incoming = listener.accept() => match incoming {
                Ok(incoming) => ui::accept(incoming, rt.next_client_id(), ui_tx.clone()),
                Err(e) => warn!("接受设置界面的连接失败：{e}"),
            },
            Some(event) = ui_events.recv() => rt.on_ui(event).await,
            Some(event) = engine_events.recv() => rt.on_engine(event),
            Some(ask) = pair_asks.recv() => rt.on_pairing(ask),
            command = control.commands.recv() => match command {
                Some(Command::Quit) | None => break,
                Some(command) => rt.on_command(command).await,
            },
            exit = rt.engine_exit() => rt.on_engine_exit(exit),
            _ = tick.tick() => rt.on_tick(),
        }
        control.summary.send_if_modified(|current| {
            let next = rt.summary();
            let changed = *current != next;
            *current = next;
            changed
        });
    }
    info!("正在退出");
    rt.stop_engine().await;
    Ok(())
}

/// 退出时删除 `ipc.json`，界面随即知道后台已经不在了。
struct RemoveOnDrop {
    path: PathBuf,
    pid: u32,
}

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        Endpoint::remove(&self.path, self.pid);
    }
}

/// 另一份后台是否已在运行：`ipc.json` 指向的端口能握手成功。
/// 在运行时请它打开设置界面并返回 `true`，当前进程应当退出。
pub async fn hand_over_to_running(store: &Store) -> bool {
    use ferry_ipc::{Notice, Request};

    let Ok(Some(endpoint)) = Endpoint::load(&store.ipc_endpoint_path()) else {
        return false;
    };
    let Ok(mut conn) = ferry_ipc::connect(&endpoint).await else {
        return false;
    };
    if conn.send(&Request::OpenSettings).await.is_err() {
        return false;
    }
    // 读掉对方随后推来的状态再断开：直接退出会让系统用 RST 断开连接，
    // 对方可能还没读到这条请求就收到了连接重置。
    let drain = async { while let Ok(Some(_)) = conn.recv::<Notice>().await {} };
    let _ = tokio::time::timeout(Duration::from_millis(300), drain).await;
    true
}
