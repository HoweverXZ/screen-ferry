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

/// 登录时执行的命令：本程序加上 `--autostart`。
fn autostart_command() -> Option<Vec<String>> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    Some(vec![exe.display().to_string(), "--autostart".into()])
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
pub(crate) use ferry_platform_macos::MacEventLoop as NativeEventLoop;
#[cfg(target_os = "macos")]
pub(crate) use mac::NativeHost;

#[cfg(target_os = "macos")]
mod mac {
    use std::process::Command;
    use std::sync::atomic::{AtomicBool, Ordering};

    use ferry_engine::resident::Host;
    use ferry_platform::{Autostart, Error, Permissions, Result};
    use ferry_platform_macos::{
        MacAutostart, MacCapture, MacClipboard, MacDisplays, MacOverlay, MacPermissions,
    };
    use ferry_types::Role;
    use tracing::warn;

    use super::{Unsupported, autostart_command, exe_dir, settings_app};

    const NO_INJECT: &str = "这台 Mac 暂时只能控制别的电脑，还不能被控制";
    /// LaunchAgent 的标识，与安装包的 bundle id 一致。
    const BUNDLE_ID: &str = "io.github.icloudza.screenferry";

    pub(crate) struct NativeHost {
        autostart: Option<MacAutostart>,
        /// 是否已经向系统申请过辅助功能权限。
        prompted: AtomicBool,
    }

    impl NativeHost {
        pub(crate) fn new() -> Self {
            let autostart =
                autostart_command().and_then(|args| MacAutostart::new(BUNDLE_ID, args).ok());
            Self {
                autostart,
                prompted: AtomicBool::new(false),
            }
        }
    }

    impl Host for NativeHost {
        type Capture = MacCapture;
        type Inject = Unsupported;
        type Displays = MacDisplays;
        type Clipboard = MacClipboard;
        type Overlay = MacOverlay;

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
                // 第一次发现缺权限时向系统申请一次：系统弹出提示，并把本程序加进授权列表。
                Role::Server if !self.prompted.swap(true, Ordering::SeqCst) => {
                    MacPermissions.request_input()
                }
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

        fn overlay(&self) -> MacOverlay {
            MacOverlay::new()
        }

        fn autostart(&self) -> bool {
            self.autostart
                .as_ref()
                .is_some_and(|a| a.is_enabled().unwrap_or(false))
        }

        fn set_autostart(&self, on: bool) -> Result<()> {
            self.autostart
                .as_ref()
                .ok_or(Error::Unsupported("找不到程序的位置"))?
                .set_enabled(on)
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
            // 先申请一次，确保本程序出现在授权列表里。
            let _ = MacPermissions.request_input();
            let url =
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";
            if let Err(e) = Command::new("open").arg(url).spawn() {
                warn!("打开系统设置失败：{e}");
            }
        }
    }
}

#[cfg(target_os = "windows")]
pub(crate) use ferry_platform_windows::WinEventLoop as NativeEventLoop;
#[cfg(target_os = "windows")]
pub(crate) use win::NativeHost;

#[cfg(target_os = "windows")]
mod win {
    use std::process::Command;

    use ferry_engine::resident::Host;
    use ferry_platform::{Autostart, Error, Permissions, Result};
    use ferry_platform_windows::{
        WinAutostart, WinClipboard, WinDisplays, WinInject, WinOverlay, WinPermissions,
    };
    use ferry_types::Role;
    use tracing::warn;

    use super::{Unsupported, autostart_command, exe_dir, settings_app};

    const NO_CAPTURE: &str = "这台电脑暂时只能被控制，还不能控制别的电脑";

    pub(crate) struct NativeHost {
        autostart: Option<WinAutostart>,
    }

    impl NativeHost {
        pub(crate) fn new() -> Self {
            // 命令行：路径带引号（可能含空格），后面跟参数。
            let autostart = autostart_command().map(|args| {
                let command = format!("\"{}\" {}", args[0], args[1..].join(" "));
                WinAutostart::new("ScreenFerry", &command)
            });
            Self { autostart }
        }
    }

    impl Host for NativeHost {
        type Capture = Unsupported;
        type Inject = WinInject;
        type Displays = WinDisplays;
        type Clipboard = WinClipboard;
        type Overlay = WinOverlay;

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

        fn overlay(&self) -> WinOverlay {
            WinOverlay::new()
        }

        fn autostart(&self) -> bool {
            self.autostart
                .as_ref()
                .is_some_and(|a| a.is_enabled().unwrap_or(false))
        }

        fn set_autostart(&self, on: bool) -> Result<()> {
            self.autostart
                .as_ref()
                .ok_or(Error::Unsupported("找不到程序的位置"))?
                .set_enabled(on)
        }

        fn open_settings(&self) {
            // 安装目录：screenferry.exe 旁边的 settings\screenferry_settings.exe。
            let candidates: Vec<_> = exe_dir()
                .map(|dir| vec![dir.join("settings").join("screenferry_settings.exe")])
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
