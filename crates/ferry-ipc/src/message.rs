//! 消息定义。每条消息编码为一行 JSON，`type` 字段区分种类。
//!
//! 设置界面（Dart）手写了对应的类型，两边靠 `app/test/fixtures/ipc_samples.json` 中的样例保持一致：
//! 改动这里的任何结构，都要同时改 Dart 侧并重新生成样例，见 `docs/ipc.md`。

use ferry_types::{Role, Side, Theme};
use serde::{Deserialize, Serialize};

/// 协议版本。不兼容的改动才递增。
pub const VERSION: u32 = 1;

/// 单行消息的上限。状态快照通常只有几 KB。
pub const MAX_LINE: usize = 1 << 20;

/// 界面 → 后台。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    /// 第一条消息：令牌与界面使用的协议版本。
    Hello { token: String, version: u32 },
    /// 暂停或继续共享。
    SetSharing { on: bool },
    /// 修改设置，只含要改的字段。
    UpdateSettings { settings: SettingsPatch },
    /// 删除一台已配对的设备，下次连接需要重新配对。
    Forget { fingerprint: String },
    /// 回答配对请求。
    PairReply { id: u64, accept: bool },
    /// 客户端：连接局域网里发现的一台服务端，未配对时先配对。
    Connect { fingerprint: String },
    /// 打开系统设置中授予键鼠控制权限的页面。
    OpenPermissionSettings,
    /// 清空日志。
    ClearLog,
    /// 请后台打开设置界面。另一份后台启动时用它把用户引到已在运行的这一份。
    OpenSettings,
    /// 把这些文件和文件夹（本机的绝对路径）发给当前连着的对方。结果以 `toast` 和日志告知。
    SendFiles { paths: Vec<String> },
}

/// 后台 → 界面。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Notice {
    /// 回应 `Hello`，之后立即发送 `History` 和 `State`。
    Welcome { version: u32 },
    /// 完整的状态快照。有变化时推送，最多每秒 4 次。
    State { state: Box<State> },
    /// 连接时发送一次已有的日志，最新的在前。
    History { entries: Vec<LogEntry> },
    /// 新的一条日志。
    Log { entry: LogEntry },
    /// 请用户核对配对码。
    Pairing { prompt: PairingPrompt },
    /// 配对请求已结束（任何一方已决定、超时或连接断开），关闭对应的对话框。
    PairingClosed { id: u64 },
    /// 给用户的简短提示。
    Toast { text: String },
}

/// 后台的完整状态。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub local: Local,
    /// 用户没有暂停共享。
    pub sharing: bool,
    /// 妨碍工作的问题，例如缺少权限。没有问题时为 `null`。
    pub problem: Option<Problem>,
    pub link: Link,
    /// 最近一次剪贴板同步。
    pub clipboard: Option<ClipboardSync>,
    /// 已配对的设备。
    pub peers: Vec<PeerInfo>,
    /// 客户端在局域网里发现的服务端；服务端上总为空。
    pub found: Vec<FoundDevice>,
    pub settings: Settings,
    /// 最近 48 小时每小时的活动，只列出有活动的小时，按时间先后排列。
    pub activity: Vec<HourActivity>,
}

/// 本机。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Local {
    pub name: String,
    /// 完整指纹，64 位十六进制。
    pub fingerprint: String,
    /// 显示用的短指纹，例如 `7AB0-5CB2-38FB-D603`。
    pub short: String,
    pub role: Role,
    pub data_dir: String,
    /// 后台的版本号。
    pub version: String,
    /// 服务端正在监听的端口；客户端、或还没开始监听时为 `null`。
    #[serde(default)]
    pub port: Option<u16>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Problem {
    pub kind: ProblemKind,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProblemKind {
    /// 缺少键鼠控制权限，界面提供去系统设置授权的按钮。
    Permission,
    /// 端口被占用。
    PortInUse,
    Other,
}

/// 与对方的连接。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Link {
    pub status: LinkStatus,
    /// 已连接时是对方，其余时候为 `null`。
    pub peer: Option<PeerRef>,
    /// 连上的时刻（Unix 毫秒）。
    pub since: Option<i64>,
    pub cursor_on_peer: bool,
    /// 最近至多 32 秒、每秒一个的平均往返延迟（毫秒），先旧后新。
    pub rtt_ms: Vec<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkStatus {
    /// 用户暂停了共享。
    Paused,
    /// 引擎正在启动或重启。
    Starting,
    /// 服务端：等待对方连接。
    Waiting,
    /// 客户端：在局域网里寻找服务端。
    Searching,
    /// 客户端：正在连接。
    Connecting,
    Connected,
    /// 因为 `problem` 无法工作。
    Stopped,
}

/// 连接中的对方。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerRef {
    pub name: String,
    pub fingerprint: String,
    pub short: String,
    /// 对方的 IP 地址。
    pub addr: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipboardSync {
    /// 从本机同步到对方为 `true`，从对方同步到本机为 `false`。
    pub to_peer: bool,
    pub peer: String,
    /// 是纯文本时为字符数，否则为 `null`。
    pub chars: Option<u32>,
    pub bytes: u64,
    /// 纯文本的开头一段，用于显示；其他内容为空。
    pub preview: String,
    /// Unix 毫秒。
    pub at: i64,
}

/// 一台已配对的设备。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerInfo {
    pub name: String,
    pub fingerprint: String,
    pub short: String,
    pub online: bool,
    /// 本次运行中最后一次断开的时刻（Unix 毫秒）；本次运行没连过为 `null`。
    pub last_seen: Option<i64>,
}

/// 局域网里发现的一台服务端。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoundDevice {
    pub name: String,
    pub fingerprint: String,
    pub short: String,
    pub addr: String,
    pub paired: bool,
}

/// 用户可以修改的设置。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub name: String,
    pub side: Side,
    pub swap_cmd_ctrl: bool,
    pub announce: bool,
    pub clipboard: bool,
    pub server: String,
    /// 登录时启动。以系统中的登记为准。
    pub autostart: bool,
    /// 设置界面的配色与强调色，由后台保存，重启后不变。
    pub theme: Theme,
    pub accent: u8,
}

/// 要修改的设置；为 `null` 或缺少的字段不变。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SettingsPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub side: Option<Side>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub swap_cmd_ctrl: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub announce: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clipboard: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub autostart: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<Theme>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accent: Option<u8>,
}

/// 一小时内的活动。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HourActivity {
    /// 这一小时开始的时刻（Unix 秒，整点）。界面按本地时区归到“今天”。
    pub hour: i64,
    /// 光标切到对方的次数。
    pub switches: u32,
    /// 剪贴板同步的次数。
    pub clips: u32,
    /// 这一小时内保持连接的秒数。
    pub connected_secs: u32,
}

/// 一条日志。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogEntry {
    pub id: u64,
    pub kind: LogKind,
    pub icon: LogIcon,
    pub text: String,
    pub detail: String,
    /// Unix 毫秒。
    pub at: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogKind {
    Cursor,
    Clip,
    Link,
    File,
}

/// 日志左侧的图标。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogIcon {
    Power,
    Link,
    Unlink,
    Lock,
    Cursor,
    Clipboard,
    Pause,
    Play,
    Layout,
    Radar,
    Warning,
    Settings,
    File,
}

/// 请用户核对的配对码。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingPrompt {
    pub id: u64,
    pub peer_name: String,
    pub fingerprint: String,
    pub short: String,
    /// 6 位数字，例如 `"482 913"`。
    pub code: String,
}
