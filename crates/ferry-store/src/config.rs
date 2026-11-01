//! `config.toml`：用户设置。第一行是 `schema_version`，以后靠它做迁移。

use std::path::Path;

use ferry_types::Config;
use serde::Deserialize;

use crate::error::{Error, Result};

/// 读出来的设置，以及迁移还要用到的旧字段。
pub(crate) struct Parsed {
    pub(crate) config: Config,
    /// 版本 3 及以前的 `swap_cmd_ctrl`（服务端：把 Command 当作对方的 Ctrl）。
    /// 为 `false` 时，迁移把每台已配对设备的修饰键方案设为“原样”，见 [`Config::SCHEMA_VERSION`]。
    pub(crate) legacy_swap_cmd_ctrl: Option<bool>,
}

/// 旧版本里有、现在没有了的字段。
#[derive(Deserialize)]
struct Legacy {
    #[serde(default)]
    swap_cmd_ctrl: Option<bool>,
}

pub(crate) fn parse(path: &Path, data: &[u8]) -> Result<Parsed> {
    let format = |detail: String| Error::Format {
        path: path.to_owned(),
        detail,
    };
    let text = std::str::from_utf8(data).map_err(|_| format("不是 UTF-8 文本".into()))?;
    let config: Config = toml::from_str(text).map_err(|e| format(e.to_string()))?;
    if config.schema_version > Config::SCHEMA_VERSION {
        return Err(Error::TooNew {
            path: path.to_owned(),
            found: config.schema_version,
        });
    }
    // 版本 1 → 2：只新增了 `layout`（缺省为空），`side` 原样沿用，读进来就是等价的版本 2。
    // 版本 2 → 3：只新增了 `clipboard_limit_mb`（缺省 32，与之前固定的上限相同），读进来即等价。
    // 版本 3 → 4：`swap_cmd_ctrl` 改成每台设备的修饰键方案，由调用方按已配对的设备展开；
    // 新增的 `mouse_dpi`、`devices` 取缺省值。
    let legacy_swap_cmd_ctrl = if config.schema_version < 4 {
        toml::from_str::<Legacy>(text)
            .map_err(|e| format(e.to_string()))?
            .swap_cmd_ctrl
    } else {
        None
    };
    Ok(Parsed {
        config: Config {
            schema_version: Config::SCHEMA_VERSION,
            ..config
        },
        legacy_swap_cmd_ctrl,
    })
}

pub(crate) fn render(config: &Config) -> String {
    toml::to_string(config).expect("字符串、布尔值、整数和它们组成的表总能写成 TOML")
}
