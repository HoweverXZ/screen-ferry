//! 消息定义。每条消息编码为一行 JSON，`type` 字段区分种类。
//!
//! 设置界面（Dart）手写了对应的类型，两边靠 `app/test/fixtures/ipc_samples.json` 中的样例保持一致：
//! 改动这里的任何结构，都要同时改 Dart 侧并重新生成样例，见 `docs/ipc.md`。

use ferry_types::{DeviceSettings, Placement, Role, Side, Theme};
use serde::{Deserialize, Serialize};

/// 协议版本。不兼容的改动才递增。
pub const VERSION: u32 = 2;

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
    /// 换上一台设备（`device.fingerprint`）的单独设置，整份替换。全是缺省值即恢复默认。
    SetDevice { device: DeviceSettings },
    /// 删除一台已配对的设备，下次连接需要重新配对。
    Forget { fingerprint: String },
    /// 回答配对请求。
    PairReply { id: u64, accept: bool },
    /// 客户端：连接局域网里发现的一台服务端，未配对时先配对。只连这一次，不会把地址写死。
    Connect { fingerprint: String },
    /// 客户端：按地址连接一台服务端（IP、IP:端口、电脑名或电脑名:端口，不写端口用 24810），
    /// 未配对时先配对。结果以 `dialed` 告知。
    ConnectAddress { address: String },
    /// 服务端：邀请这个地址上的电脑（被控制的一方）连过来，端口不写用 24811。结果以 `dialed` 告知。
    Invite { address: String },
    /// 打开系统设置中授予键鼠控制权限的页面。
    OpenPermissionSettings,
    /// 清空日志。
    ClearLog,
    /// 请后台打开设置界面。另一份后台启动时用它把用户引到已在运行的这一份。
    OpenSettings,
    /// 请连着的那台设备 `fingerprint` 打开它的设置界面，并翻到本机的设置页。
    /// 本机被它控制时，滚动、按键等设置要在它那边改。结果以 `toast` 告知。
    OpenPeerSettings { fingerprint: String },
    /// 把这些文件和文件夹（本机的绝对路径）发给连着的设备 `to`（证书指纹）。
    /// 不指定时发给光标最近去过的那台，只连着一台时就是它。结果以 `toast` 和日志告知。
    SendFiles {
        paths: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to: Option<String>,
    },
    /// 请后台退出：停下共享（归还键鼠、松开按键），通知界面一起关闭，然后退出。
    /// 安装程序升级、卸载前用它让正在运行的 ScreenFerry 放开程序文件。
    Quit,
    /// 读运行日志：`since` 为空时取最近的一段，否则取上次 `run_log` 给的位置之后新写的。
    /// 结果以 `run_log` 告知。
    ReadRunLog {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        since: Option<u64>,
    },
    /// 把诊断文件存到“下载”文件夹，结果以 `diagnostics_exported` 告知，出错时以 `toast`。
    ExportDiagnostics,
    /// 取诊断摘要（版本、系统和最近 200 行运行日志），结果以 `diagnostics_summary` 告知，界面复制到剪贴板。
    CopyDiagnostics,
    /// 在访达或资源管理器里显示最近导出的诊断文件。
    RevealDiagnostics,
    /// 打开运行日志所在的文件夹。
    OpenLogsFolder,
    /// 打开或关闭详细记录（运行日志多记调试信息）。重启后总是关着。
    SetVerbose { on: bool },
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
    /// 请界面翻到这台设备（证书指纹）的设置页。另一台设备请本机打开设置时发送。
    ShowDevice { fingerprint: String },
    /// 按地址连接（`invite` 为假）或邀请（为真）的结果。`error` 为 `null` 表示成了：
    /// 连上了、开始核对配对码，或者对方接受了邀请、正在连过来。
    Dialed {
        address: String,
        invite: bool,
        error: Option<DialError>,
    },
    /// 后台要退出了（托盘里的“退出”或 `quit` 命令），界面随之关闭。之后连接断开。
    Quit,
    /// 运行日志的一段，回应 `read_run_log`。`fresh` 为真时界面整个替换（第一次读，或者日志换了一卷），
    /// 否则接在已有的后面。`next` 是下次读的位置；`bytes`、`lines_total` 是当前这卷的大小和行数。
    RunLog {
        lines: Vec<RunLogLine>,
        next: u64,
        fresh: bool,
        bytes: u64,
        lines_total: u64,
    },
    /// 诊断文件存好了：完整路径和大小。
    DiagnosticsExported { path: String, bytes: u64 },
    /// 诊断摘要，界面复制到剪贴板。
    DiagnosticsSummary { text: String },
}

/// 运行日志的一行。设备名、文件名等照原样显示（导出时才换成代号）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunLogLine {
    /// 本机时间 `HH:MM:SS`；旧格式的行没有时间，为空。
    pub at: String,
    pub level: RunLogLevel,
    pub text: String,
}

/// 运行日志一行的级别。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunLogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

/// 按地址连接、邀请为什么没成。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DialError {
    /// 地址写得不对，或者电脑名找不到。
    BadAddress,
    /// 那个地址上没有在等待的 ScreenFerry：没开、角色不对，或者版本太旧不支持邀请。
    Refused,
    /// 连不上：网络不通、被防火墙挡住，或者超时。
    Unreachable,
    /// 那个地址上不是 ScreenFerry。
    NotScreenFerry,
    /// 对方的版本与本机不兼容。
    Incompatible,
    /// 邀请时对方正连着另一台电脑。
    Busy,
    /// 本机现在的角色做不了这件事（例如服务端按地址连接）。
    WrongRole,
}

/// 后台的完整状态。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub local: Local,
    /// 用户没有暂停共享。
    pub sharing: bool,
    /// 妨碍工作的问题，例如缺少权限。没有问题时为 `null`。
    pub problem: Option<Problem>,
    /// 连接摘要：连着几台时指光标所在（否则最近连上）的那台。每台设备是否连着见 `peers`。
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
    /// 本机系统：`macos`、`windows` 等。旧版后台没有这个字段，为空字符串。
    #[serde(default)]
    pub platform: String,
    /// macOS：系统设置里开着“自然滚动”。其他系统为 `null`。
    #[serde(default)]
    pub natural_scrolling: Option<bool>,
    /// 本机的局域网地址（IPv4），常见网段在前。另一台电脑按地址连接时填它。
    #[serde(default)]
    pub addresses: Vec<String>,
    /// 客户端：等待邀请的端口；服务端为 `null`。
    #[serde(default)]
    pub invite_port: Option<u16>,
    /// 本机系统和版本，例如“macOS 15.6 · arm64”。不知道时为空。
    #[serde(default)]
    pub system: String,
    /// 详细记录从什么时候开着（Unix 毫秒）；关着时为 `null`。
    #[serde(default)]
    pub verbose_since: Option<i64>,
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
    /// 文字的开头一段，用于显示；没有文字时为空。
    pub preview: String,
    /// 带格式（HTML 或 RTF）。
    #[serde(default)]
    pub rich: bool,
    /// 有图片时的尺寸和预览；没有图片为 `null`。
    #[serde(default)]
    pub image: Option<ClipboardImage>,
    /// Unix 毫秒。
    pub at: i64,
}

/// 剪贴板里的一张图片。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipboardImage {
    pub width: u32,
    pub height: u32,
    /// 预览用的 PNG 文件（本机绝对路径）；没能存下来时为 `null`。
    #[serde(default)]
    pub path: Option<String>,
}

fn default_clipboard_limit() -> u32 {
    32
}

fn default_mouse_dpi() -> u32 {
    1600
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
    /// 服务端：这台设备在本机哪一侧；客户端为 `null`。
    /// 同一侧的先后见 `settings.layout`，没有列在里面的排在后面。
    #[serde(default)]
    pub side: Option<Side>,
    /// 光标正在这台设备上。
    #[serde(default)]
    pub cursor: bool,
    /// 本机给它起的名字（见 `settings.devices`），空为没起。`name` 总是它自己报的名字。
    #[serde(default)]
    pub alias: String,
    /// 它是什么系统：`macos`、`windows`；本次运行还没连上过、不知道时为空字符串。
    #[serde(default)]
    pub platform: String,
    /// 上次连上时它的地址：`IP:端口`，只知道 IP 时就是 IP；没连上过为空字符串。
    #[serde(default)]
    pub addr: String,
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
    /// 没有在 `layout` 里单独安排的设备放在本机哪一侧。
    pub side: Side,
    /// 单独安排过位置的设备，按先后排列：同一侧的几台按这个先后平分那条边
    /// （左右两侧从上到下，上下两侧从左到右）。
    #[serde(default)]
    pub layout: Vec<Placement>,
    pub announce: bool,
    pub clipboard: bool,
    /// 剪贴板单项（一种格式）的大小上限，单位 MB，1–64。
    #[serde(default = "default_clipboard_limit")]
    pub clipboard_limit_mb: u32,
    /// 本机鼠标的 DPI，100–12800。每台设备的光标速度以它为准。
    #[serde(default = "default_mouse_dpi")]
    pub mouse_dpi: u32,
    pub server: String,
    /// 登录时启动。以系统中的登记为准。
    pub autostart: bool,
    /// 设置界面的配色与强调色，由后台保存，重启后不变。
    pub theme: Theme,
    pub accent: u8,
    /// 单独设置过的设备；没列出的用缺省设置。
    #[serde(default)]
    pub devices: Vec<DeviceSettings>,
}

/// 要修改的设置；为 `null` 或缺少的字段不变。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SettingsPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 改变本机的角色：控制别的电脑，还是被控制。会断开现有的连接、重新开始。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub side: Option<Side>,
    /// 整份替换。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<Vec<Placement>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub announce: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clipboard: Option<bool>,
    /// 1–64，超出的收进范围内。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clipboard_limit_mb: Option<u32>,
    /// 100–12800，超出的收进范围内。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mouse_dpi: Option<u32>,
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
