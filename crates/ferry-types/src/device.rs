//! 每台设备的单独设置：本机控制它时按键怎么映射、光标和滚轮怎么换算、切过去前要不要在边上停一下，
//! 以及和它之间同不同步剪贴板、能不能把文件拖给它、在本机上叫它什么。
//!
//! 保存在本机的 `config.toml` 里，按证书指纹区分（ADR 0009）。

use std::ops::RangeInclusive;

use serde::{Deserialize, Serialize};

/// 修饰键方案。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModifierPreset {
    /// 按本机的习惯：两台一台以 Command 为主、一台不是时，互换 Command（Windows 键）与 Ctrl，
    /// 在 Windows 上按 ⌘C 就是复制；两台是同一种键盘时原样发送。
    #[default]
    Habit,
    /// 原样发送：⌘ 到了 Windows 上就是 Windows 键。
    Plain,
    /// 按 [`DeviceSettings::custom_modifiers`] 逐个指定。
    Custom,
}

/// 修饰键要变成的键。左右两侧各自对应：右边的键变成右边的键。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModifierTarget {
    Ctrl,
    Shift,
    Alt,
    /// Windows 键 / Command 键。
    Gui,
    CapsLock,
    /// 不发送。
    None,
}

/// 四个修饰键各自变成什么。键名按 USB HID：`gui` 是 Mac 的 ⌘、Windows 的 Windows 键，`alt` 是 ⌥ / Alt。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct ModifierMap {
    pub gui: ModifierTarget,
    pub ctrl: ModifierTarget,
    pub alt: ModifierTarget,
    pub caps_lock: ModifierTarget,
}

impl ModifierMap {
    /// 原样：每个键都还是它自己。
    pub const PLAIN: Self = Self {
        gui: ModifierTarget::Gui,
        ctrl: ModifierTarget::Ctrl,
        alt: ModifierTarget::Alt,
        caps_lock: ModifierTarget::CapsLock,
    };

    /// 互换 Command（Windows 键）与 Ctrl。
    pub const SWAP_GUI_CTRL: Self = Self {
        gui: ModifierTarget::Ctrl,
        ctrl: ModifierTarget::Gui,
        ..Self::PLAIN
    };
}

impl Default for ModifierMap {
    fn default() -> Self {
        Self::PLAIN
    }
}

/// 把一个键换成另一个键。键码是 USB HID 扩展用法码（见 [`KeyCode`](crate::KeyCode)）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyMapping {
    pub from: u32,
    /// 0 表示不发送：在这台设备上按这个键没有反应。
    pub to: u32,
}

/// 滚动速度。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrollSpeed {
    Slow,
    #[default]
    Normal,
    Fast,
}

impl ScrollSpeed {
    /// 滚动距离乘的倍数（百分数）。
    pub fn percent(self) -> u32 {
        match self {
            Self::Slow => 50,
            Self::Normal => 100,
            Self::Fast => 200,
        }
    }
}

/// 一台设备的单独设置。缺少的字段取缺省值。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DeviceSettings {
    /// 设备的证书指纹，64 位十六进制。
    pub fingerprint: String,
    /// 本机给它起的名字，只在本机显示；空为它自己报的名字。
    pub alias: String,
    /// 光标速度：光标在它上面时，就像接着一个这么多 DPI 的鼠标。
    /// 0 表示和本机一样（[`Config::mouse_dpi`](crate::Config::mouse_dpi)）。
    pub pointer_dpi: u32,
    /// 上下滚动的方向反过来。
    pub reverse_vertical: bool,
    /// 左右滚动的方向反过来。
    pub reverse_horizontal: bool,
    pub scroll_speed: ScrollSpeed,
    pub modifiers: ModifierPreset,
    /// 两台不是同一种键盘时，文本编辑快捷键按本机的习惯转换，例如在 Windows 上按 ⌘← 到行首。
    pub text_shortcuts: bool,
    /// 光标要在朝向它的边上停这么久（毫秒）才切过去，防止误切。0 为立即切换。
    pub edge_delay_ms: u32,
    /// 和它之间同步剪贴板（总开关关着时不同步）。
    pub clipboard: bool,
    /// 能把文件拖给它。
    pub files: bool,
    /// `modifiers` 为 `custom` 时四个修饰键各自变成什么。
    pub custom_modifiers: ModifierMap,
    /// 其他按键映射。同一个键只看第一条。
    pub keys: Vec<KeyMapping>,
}

impl DeviceSettings {
    /// 光标速度的取值范围（DPI）。
    pub const POINTER_DPI: RangeInclusive<u32> = 100..=12800;
    /// 切过去前最多等多久（毫秒）。
    pub const MAX_EDGE_DELAY_MS: u32 = 2000;

    /// 一台设备的缺省设置。
    pub fn new(fingerprint: impl Into<String>) -> Self {
        Self {
            fingerprint: fingerprint.into(),
            ..Self::default()
        }
    }

    /// 除了指纹，其余都是缺省值。
    pub fn is_default(&self) -> bool {
        *self == Self::new(self.fingerprint.clone())
    }

    /// 把超出范围的值收进范围内。
    pub fn clamped(mut self) -> Self {
        if self.pointer_dpi != 0 {
            let range = Self::POINTER_DPI;
            self.pointer_dpi = self.pointer_dpi.clamp(*range.start(), *range.end());
        }
        self.edge_delay_ms = self.edge_delay_ms.min(Self::MAX_EDGE_DELAY_MS);
        self
    }

    /// 实际的修饰键对应关系。`differ`：两台不是同一种键盘（一台以 Command 为主、一台不是）。
    pub fn modifier_map(&self, differ: bool) -> ModifierMap {
        match self.modifiers {
            ModifierPreset::Habit if differ => ModifierMap::SWAP_GUI_CTRL,
            ModifierPreset::Habit | ModifierPreset::Plain => ModifierMap::PLAIN,
            ModifierPreset::Custom => self.custom_modifiers,
        }
    }
}

impl Default for DeviceSettings {
    fn default() -> Self {
        Self {
            fingerprint: String::new(),
            alias: String::new(),
            pointer_dpi: 0,
            reverse_vertical: false,
            reverse_horizontal: false,
            scroll_speed: ScrollSpeed::Normal,
            modifiers: ModifierPreset::Habit,
            text_shortcuts: false,
            edge_delay_ms: 0,
            clipboard: true,
            files: true,
            custom_modifiers: ModifierMap::PLAIN,
            keys: Vec::new(),
        }
    }
}
