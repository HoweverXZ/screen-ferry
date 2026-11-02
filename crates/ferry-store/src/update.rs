//! 缓存目录下的 `updates/state.toml`：版本更新的状态（ADR 0014）。
//!
//! ```toml
//! seen_published = 1795161600   # 见过的最新一份更新清单的发布时间（Unix 秒），防回滚
//! dismissed = "0.2.0"           # 仪表盘横幅上点过“稍后”的版本
//!
//! [installing]                  # 正在安装的更新，新版本启动后据此提示“已更新到 …”
//! from = "0.1.0"
//! to = "0.2.0"
//! open_settings = true          # 安装前设置界面开着，装好后重新打开
//! ```
//!
//! 缓存目录可以随时清空：这个文件没了，最多是再收起一次横幅，防回滚靠“只装更新的版本”兜底。

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateState {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seen_published: Option<i64>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub dismissed: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installing: Option<Installing>,
}

/// 正在安装的更新。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Installing {
    pub from: String,
    pub to: String,
    pub open_settings: bool,
}
