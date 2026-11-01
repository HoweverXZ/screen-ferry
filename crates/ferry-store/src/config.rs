//! `config.toml`：用户设置。第一行是 `schema_version`，以后靠它做迁移。

use std::path::Path;

use ferry_types::Config;

use crate::error::{Error, Result};

pub(crate) fn parse(path: &Path, data: &[u8]) -> Result<Config> {
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
    Ok(Config {
        schema_version: Config::SCHEMA_VERSION,
        ..config
    })
}

pub(crate) fn render(config: &Config) -> String {
    toml::to_string(config).expect("字符串、布尔值和整数总能写成 TOML")
}
