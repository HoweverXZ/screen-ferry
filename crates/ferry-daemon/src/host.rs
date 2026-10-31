//! 本机能力：按目标平台创建平台对象，以及只有应用才知道怎么做的事（打开设置界面、读计算机名）。

use std::path::PathBuf;

use ferry_platform::{CaptureSink, Error, InputCapture, InputInject, Result};
use ferry_types::{InputEvent, LockKeys, Point};

/// 本机还做不到的那个方向：目前 macOS 只能控制别人，Windows 只能被控制。
pub(crate) struct Unsupported(pub(crate) &'static str);

impl InputCapture for Unsupported {
    fn start(&mut self, _sink: CaptureSink) -> Result<()> {
        Err(Error::Unsupported(self.0))
    }
    fn grab(&self) -> Result<()> {
        Err(Error::Unsupported(self.0))
    }
    fn release(&self, _at: Point) -> Result<()> {
        Err(Error::Unsupported(self.0))
    }
    fn lock_keys(&self) -> LockKeys {
        LockKeys::empty()
    }
}

impl InputInject for Unsupported {
    fn inject(&mut self, _event: &InputEvent) -> Result<()> {
        Err(Error::Unsupported(self.0))
    }
    fn sync_locks(&mut self, _wanted: LockKeys, _mask: LockKeys) -> Result<()> {
        Err(Error::Unsupported(self.0))
    }
}

/// 指定设置界面位置的环境变量，调试时用。
const SETTINGS_ENV: &str = "SCREENFERRY_SETTINGS_APP";

/// 按顺序找设置界面：环境变量、安装包里的位置、开发时 Flutter 的构建产物。
fn settings_app(candidates: &[PathBuf]) -> Option<PathBuf> {
    std::env::var_os(SETTINGS_ENV)
        .map(PathBuf::from)
        .into_iter()
        .chain(candidates.iter().cloned())
        .find(|p| p.exists())
}

/// 可执行文件所在的目录。
fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()?
        .canonicalize()
        .ok()?
        .parent()
        .map(PathBuf::from)
}

#[cfg(target_os = "macos")]
pub(crate) use mac::NativeHost;

#[cfg(target_os = "macos")]
mod mac {
    use std::process::Command;

    use ferry_engine::resident::Host;
    use ferry_platform::{Error, Permissions, Result};
    use ferry_platform_macos::{MacCapture, MacClipboard, MacDisplays, MacPermissions};
    use ferry_types::Role;
    use tracing::warn;

    use super::{Unsupported, exe_dir, settings_app};

    const NO_INJECT: &str = "这台 Mac 暂时只能控制别的电脑，还不能被控制";

    pub(crate) struct NativeHost;

    impl Host for NativeHost {
        type Capture = MacCapture;
        type Inject = Unsupported;
        type Displays = MacDisplays;
        type Clipboard = MacClipboard;

        fn device_name(&self) -> String {
            Command::new("scutil")
                .args(["--get", "ComputerName"])
                .output()
                .ok()
                .and_then(|out| String::from_utf8(out.stdout).ok())
                .map(|name| name.trim().to_owned())
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| "Mac".into())
        }

        fn check_permissions(&self, role: Role) -> Result<()> {
            match role {
                Role::Server => MacPermissions.check_input(),
                Role::Client => Err(Error::Unsupported(NO_INJECT)),
            }
        }

        fn capture(&self) -> Result<MacCapture> {
            Ok(MacCapture::new())
        }

        fn inject(&self) -> Result<Unsupported> {
            Err(Error::Unsupported(NO_INJECT))
        }

        fn displays(&self) -> MacDisplays {
            MacDisplays
        }

        fn clipboard(&self) -> MacClipboard {
            MacClipboard::new()
        }

        fn autostart(&self) -> bool {
            false
        }

        fn set_autostart(&self, _on: bool) -> Result<()> {
            Err(Error::Unsupported("开机自启即将支持"))
        }

        fn open_settings(&self) {
            let candidates: Vec<_> = exe_dir()
                .map(|dir| {
                    vec![
                        // 安装包：ScreenFerry.app/Contents/MacOS/screenferry 旁边的 Helpers。
                        dir.join("../Helpers/ScreenFerry Settings.app"),
                        // 开发：target/<profile>/screenferry 与 app/ 的构建产物。
                        dir.join("../../app/build/macos/Build/Products/Release/ScreenFerry.app"),
                        dir.join("../../app/build/macos/Build/Products/Debug/ScreenFerry.app"),
                    ]
                })
                .unwrap_or_default();
            match settings_app(&candidates) {
                // `open` 会把已经打开的设置界面带到前台。
                Some(app) => {
                    if let Err(e) = Command::new("open").arg(&app).spawn() {
                        warn!("打开设置界面失败：{e}");
                    }
                }
                None => warn!("找不到设置界面，可以用环境变量 SCREENFERRY_SETTINGS_APP 指定"),
            }
        }

        fn open_permission_settings(&self) {
            let url =
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";
            if let Err(e) = Command::new("open").arg(url).spawn() {
                warn!("打开系统设置失败：{e}");
            }
        }
    }
}

#[cfg(target_os = "windows")]
pub(crate) use win::NativeHost;

#[cfg(target_os = "windows")]
mod win {
    use std::process::Command;

    use ferry_engine::resident::Host;
    use ferry_platform::{Error, Permissions, Result};
    use ferry_platform_windows::{WinClipboard, WinDisplays, WinInject, WinPermissions};
    use ferry_types::Role;
    use tracing::warn;

    use super::{Unsupported, exe_dir, settings_app};

    const NO_CAPTURE: &str = "这台电脑暂时只能被控制，还不能控制别的电脑";

    pub(crate) struct NativeHost;

    impl Host for NativeHost {
        type Capture = Unsupported;
        type Inject = WinInject;
        type Displays = WinDisplays;
        type Clipboard = WinClipboard;

        fn device_name(&self) -> String {
            std::env::var("COMPUTERNAME")
                .ok()
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| "Windows".into())
        }

        fn check_permissions(&self, role: Role) -> Result<()> {
            match role {
                Role::Client => WinPermissions.check_input(),
                Role::Server => Err(Error::Unsupported(NO_CAPTURE)),
            }
        }

        fn capture(&self) -> Result<Unsupported> {
            Err(Error::Unsupported(NO_CAPTURE))
        }

        fn inject(&self) -> Result<WinInject> {
            Ok(WinInject::new())
        }

        fn displays(&self) -> WinDisplays {
            WinDisplays::new()
        }

        fn clipboard(&self) -> WinClipboard {
            WinClipboard::new()
        }

        fn autostart(&self) -> bool {
            false
        }

        fn set_autostart(&self, _on: bool) -> Result<()> {
            Err(Error::Unsupported("开机自启即将支持"))
        }

        fn open_settings(&self) {
            let candidates: Vec<_> = exe_dir()
                .map(|dir| vec![dir.join("ScreenFerry Settings").join("ScreenFerry.exe")])
                .unwrap_or_default();
            match settings_app(&candidates) {
                Some(exe) => {
                    if let Err(e) = Command::new(exe).spawn() {
                        warn!("打开设置界面失败：{e}");
                    }
                }
                None => warn!("找不到设置界面，可以用环境变量 SCREENFERRY_SETTINGS_APP 指定"),
            }
        }

        fn open_permission_settings(&self) {}
    }
}
