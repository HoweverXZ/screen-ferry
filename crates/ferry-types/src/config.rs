//! 配置结构：`config.toml` 的内容。只有后台写入，读写由 `ferry-store` 负责，见 ADR 0004。

use serde::{Deserialize, Serialize};

use crate::Side;

/// 本机在一对设备中的角色。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// 键鼠在本机，光标可以切到对方。
    Server,
    /// 被对方控制。
    Client,
}

impl Role {
    /// 本机系统缺省的角色：macOS 当服务端，Windows 当客户端。两边都可以在设置里改。
    pub fn native() -> Self {
        if std::env::consts::OS == "windows" {
            Self::Client
        } else {
            Self::Server
        }
    }
}

/// 本机的快捷键是否以 Command 键为主（只有 macOS 是）。
///
/// 两台电脑一台是一台不是时，才需要互换 Command 与 Ctrl。
pub fn uses_command_key() -> bool {
    std::env::consts::OS == "macos"
}

/// 设置界面的配色。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

/// `config.toml`。缺少的字段取缺省值，不认识的字段忽略。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// 必须是文件的第一个字段，以后靠它做迁移。
    pub schema_version: u32,
    /// 设备名，空为计算机名。
    pub name: String,
    pub role: Role,
    /// 服务端：没有在 `layout` 里单独安排的设备放在本机哪一侧。
    pub side: Side,
    /// 服务端：把 Command 当作对方的 Ctrl。
    pub swap_cmd_ctrl: bool,
    /// 服务端：在局域网里广播本机。
    pub announce: bool,
    /// 同步剪贴板。
    pub clipboard: bool,
    /// 客户端：服务端地址，空为在局域网里寻找。
    pub server: String,
    /// 设置界面的配色。
    pub theme: Theme,
    /// 设置界面的强调色，界面预设颜色的序号。
    pub accent: u8,
    /// 服务端：单独安排过位置的设备，按先后排列。同一侧的几台按这个先后平分那条边，
    /// 没有列出的设备放在 `side` 一侧、排在列出的后面。
    ///
    /// 写成 TOML 时是表格数组，必须放在最后。
    pub layout: Vec<Placement>,
}

impl Config {
    /// 当前的配置结构版本。
    ///
    /// - 1：第一版。
    /// - 2：新增 `layout`；`side` 改为“没有单独安排的设备放在哪一侧”，只有一台设备时与版本 1 等价。
    pub const SCHEMA_VERSION: u32 = 2;
}

/// 一台设备放在服务端的哪一侧。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placement {
    /// 设备的证书指纹，64 位十六进制。
    pub fingerprint: String,
    pub side: Side,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: Self::SCHEMA_VERSION,
            name: String::new(),
            role: Role::native(),
            side: Side::Right,
            swap_cmd_ctrl: true,
            announce: true,
            clipboard: true,
            server: String::new(),
            theme: Theme::System,
            accent: 0,
            layout: Vec::new(),
        }
    }
}
