//! 开机自启：`Autostart` 的 Windows 实现。
//!
//! 在 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` 下登记一条命令，登录时由系统启动后台。
//! 只影响当前用户，不需要管理员权限。

use ferry_platform::{Autostart, Error, Result};
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, WIN32_ERROR};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SAM_FLAGS, REG_SZ, RegCloseKey,
    RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
};
use windows::core::{PCWSTR, w};

const RUN: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");

pub struct WinAutostart {
    /// 以 0 结尾的 UTF-16 值名。
    name: Vec<u16>,
    command: String,
}

impl WinAutostart {
    /// `name` 是登记项的名字，`command` 是登录时执行的完整命令行（路径含空格时要带引号）。
    pub fn new(name: &str, command: &str) -> Self {
        Self {
            name: name.encode_utf16().chain([0]).collect(),
            command: command.to_owned(),
        }
    }

    fn open(&self, access: REG_SAM_FLAGS) -> Result<RunKey> {
        let mut key = HKEY::default();
        // SAFETY: RUN 是以 0 结尾的常量字符串，key 指向有效的输出位置。
        check("RegOpenKeyExW", unsafe {
            RegOpenKeyExW(HKEY_CURRENT_USER, RUN, None, access, &mut key)
        })?;
        Ok(RunKey(key))
    }
}

/// 用完自动关闭的注册表键。
struct RunKey(HKEY);

impl Drop for RunKey {
    fn drop(&mut self) {
        // SAFETY: 键由 RegOpenKeyExW 打开，只关闭一次。
        let _ = unsafe { RegCloseKey(self.0) };
    }
}

impl Autostart for WinAutostart {
    fn is_enabled(&self) -> Result<bool> {
        let key = self.open(KEY_QUERY_VALUE)?;
        // SAFETY: 值名以 0 结尾；只问是否存在，不读数据。
        let found =
            unsafe { RegQueryValueExW(key.0, PCWSTR(self.name.as_ptr()), None, None, None, None) };
        match found {
            e if e == ERROR_FILE_NOT_FOUND => Ok(false),
            e => check("RegQueryValueExW", e).map(|()| true),
        }
    }

    fn set_enabled(&self, enabled: bool) -> Result<()> {
        let key = self.open(KEY_SET_VALUE)?;
        let name = PCWSTR(self.name.as_ptr());
        if enabled {
            let data: Vec<u8> = self
                .command
                .encode_utf16()
                .chain([0])
                .flat_map(u16::to_le_bytes)
                .collect();
            // SAFETY: 值名以 0 结尾，数据是以 0 结尾的 UTF-16 字符串。
            check("RegSetValueExW", unsafe {
                RegSetValueExW(key.0, name, None, REG_SZ, Some(&data))
            })
        } else {
            // SAFETY: 值名以 0 结尾。
            match unsafe { RegDeleteValueW(key.0, name) } {
                e if e == ERROR_FILE_NOT_FOUND => Ok(()),
                e => check("RegDeleteValueW", e),
            }
        }
    }
}

fn check(call: &'static str, result: WIN32_ERROR) -> Result<()> {
    result.ok().map_err(|e| Error::Os {
        call,
        detail: e.to_string(),
    })
}
