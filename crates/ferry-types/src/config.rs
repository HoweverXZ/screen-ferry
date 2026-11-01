//! 配置结构：`config.toml` 的内容。只有后台写入，读写由 `ferry-store` 负责，见 ADR 0004。

use serde::{Deserialize, Serialize};

use crate::Side;
use crate::device::DeviceSettings;

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
    /// 本机鼠标的 DPI，每台设备的光标速度以它为准换算。
    pub mouse_dpi: u32,
    /// 服务端：在局域网里广播本机。
    pub announce: bool,
    /// 同步剪贴板。
    pub clipboard: bool,
    /// 剪贴板单项（一种格式）的大小上限，单位 MB，1–64。超过的不同步。
    pub clipboard_limit_mb: u32,
    /// 客户端：服务端地址，空为在局域网里寻找。
    pub server: String,
    /// 设置界面的配色。
    pub theme: Theme,
    /// 设置界面的强调色，界面预设颜色的序号。
    pub accent: u8,
    /// 服务端：单独安排过位置的设备，按先后排列。同一侧的几台按这个先后平分那条边，
    /// 没有列出的设备放在 `side` 一侧、排在列出的后面。
    ///
    /// 写成 TOML 时是表格数组，和 `devices` 一起放在最后。
    pub layout: Vec<Placement>,
    /// 每台设备的单独设置，按指纹区分；没有列出的设备用缺省设置。
    pub devices: Vec<DeviceSettings>,
}

impl Config {
    /// 当前的配置结构版本。
    ///
    /// - 1：第一版。
    /// - 2：新增 `layout`；`side` 改为“没有单独安排的设备放在哪一侧”，只有一台设备时与版本 1 等价。
    /// - 3：新增 `clipboard_limit_mb`（缺省 32，与之前固定的上限相同）。
    /// - 4：去掉 `swap_cmd_ctrl`，改为每台设备的 `devices.modifiers`；新增 `mouse_dpi`（缺省 1600）。
    ///   版本 3 里 `swap_cmd_ctrl = false` 的，迁移时给每台已配对的设备写上 `modifiers = "plain"`。
    pub const SCHEMA_VERSION: u32 = 4;

    /// 剪贴板单项大小上限的取值范围（MB）。
    pub const CLIPBOARD_LIMIT_MB: std::ops::RangeInclusive<u32> = 1..=64;

    /// 本机鼠标 DPI 的取值范围，与每台设备的光标速度相同。
    pub const MOUSE_DPI: std::ops::RangeInclusive<u32> = DeviceSettings::POINTER_DPI;

    /// 剪贴板单项的大小上限（字节），超出范围的值收进范围内。
    pub fn clipboard_limit_bytes(&self) -> u64 {
        let range = Self::CLIPBOARD_LIMIT_MB;
        u64::from(self.clipboard_limit_mb.clamp(*range.start(), *range.end())) * 1024 * 1024
    }

    /// 本机鼠标的 DPI，超出范围的值收进范围内。
    pub fn mouse_dpi(&self) -> u32 {
        let range = Self::MOUSE_DPI;
        self.mouse_dpi.clamp(*range.start(), *range.end())
    }

    /// 一台设备（指纹的十六进制）的设置；没有单独设置过的用缺省设置。
    pub fn device(&self, fingerprint: &str) -> DeviceSettings {
        self.devices
            .iter()
            .find(|d| d.fingerprint == fingerprint)
            .cloned()
            .unwrap_or_else(|| DeviceSettings::new(fingerprint))
    }

    /// 换上一台设备的设置：已有的替换，没有的加在后面。全是缺省值时去掉这一条。
    pub fn set_device(&mut self, settings: DeviceSettings) {
        let at = self
            .devices
            .iter()
            .position(|d| d.fingerprint == settings.fingerprint);
        match (at, settings.is_default()) {
            (Some(i), true) => {
                self.devices.remove(i);
            }
            (Some(i), false) => self.devices[i] = settings,
            (None, true) => {}
            (None, false) => self.devices.push(settings),
        }
    }
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
            mouse_dpi: 1600,
            announce: true,
            clipboard: true,
            clipboard_limit_mb: 32,
            server: String::new(),
            theme: Theme::System,
            accent: 0,
            layout: Vec::new(),
            devices: Vec::new(),
        }
    }
}
