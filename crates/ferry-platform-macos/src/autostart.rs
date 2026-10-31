//! 开机自启：`Autostart` 的 macOS 实现。
//!
//! 在 `~/Library/LaunchAgents/` 放一个 LaunchAgent 描述文件，登录时由 launchd 启动后台。
//! 不需要签名，也不需要 `launchctl load`：下次登录生效。

use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use ferry_platform::{Autostart, Error, Result};

pub struct MacAutostart {
    label: String,
    args: Vec<String>,
    path: PathBuf,
}

impl MacAutostart {
    /// `label` 是 LaunchAgent 的标识（用应用的 bundle id），`args` 是登录时执行的命令及参数。
    pub fn new(label: &str, args: Vec<String>) -> Result<Self> {
        let home = std::env::var_os("HOME")
            .filter(|h| !h.is_empty())
            .ok_or(Error::Unsupported("找不到用户目录（HOME）"))?;
        Ok(Self::in_dir(
            PathBuf::from(home).join("Library/LaunchAgents"),
            label,
            args,
        ))
    }

    /// 把描述文件放在 `dir` 下，而不是用户的 LaunchAgents 目录。
    pub fn in_dir(dir: PathBuf, label: &str, args: Vec<String>) -> Self {
        Self {
            path: dir.join(format!("{label}.plist")),
            label: label.to_owned(),
            args,
        }
    }

    fn plist(&self) -> String {
        let args: String = self
            .args
            .iter()
            .map(|a| format!("\t\t<string>{}</string>\n", escape(a)))
            .collect();
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key>
	<string>{}</string>
	<key>ProgramArguments</key>
	<array>
{args}	</array>
	<key>RunAtLoad</key>
	<true/>
	<key>ProcessType</key>
	<string>Interactive</string>
	<key>LimitLoadToSessionType</key>
	<string>Aqua</string>
</dict>
</plist>
"#,
            escape(&self.label)
        )
    }
}

impl Autostart for MacAutostart {
    fn is_enabled(&self) -> Result<bool> {
        Ok(self.path.is_file())
    }

    fn set_enabled(&self, enabled: bool) -> Result<()> {
        if enabled {
            if let Some(dir) = self.path.parent() {
                fs::create_dir_all(dir).map_err(|e| os("创建 LaunchAgents 目录", e))?;
            }
            fs::write(&self.path, self.plist()).map_err(|e| os("写入 LaunchAgent", e))
        } else {
            match fs::remove_file(&self.path) {
                Err(e) if e.kind() != ErrorKind::NotFound => Err(os("删除 LaunchAgent", e)),
                _ => Ok(()),
            }
        }
    }
}

fn os(call: &'static str, e: std::io::Error) -> Error {
    Error::Os {
        call,
        detail: e.to_string(),
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
