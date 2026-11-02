//! 自己更新：`Updater` 的 macOS 实现（ADR 0014）。
//!
//! 更新包是公证过的 `ScreenFerry.app` 打成的 zip。安装时：
//!
//! 1. 解压到应用旁边的隐藏文件夹 `.ScreenFerry-update/`（和应用在同一个磁盘上，才能一步交换）；
//! 2. 新应用的签名必须满足本机这份的“指定要求”（designated requirement：同一个开发者团队、
//!    同一个 bundle id），并且通过 Gatekeeper（已公证），版本号和清单一致；
//! 3. 用 `renamex_np(RENAME_SWAP)` 一步交换新旧应用，旧的留在隐藏文件夹里作备份。
//!
//! 签名、bundle id 和位置都不变，系统授予的辅助功能权限不会丢。临时签名（ad-hoc）的开发版本
//! 没有固定的指定要求，不能自己更新。

use std::ffi::{CString, OsStr};
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use ferry_platform::{Error, Prepared, Result, Updater};

const CODESIGN: &str = "/usr/bin/codesign";
const DITTO: &str = "/usr/bin/ditto";
const OPEN: &str = "/usr/bin/open";
const PLUTIL: &str = "/usr/bin/plutil";
const SCUTIL: &str = "/usr/sbin/scutil";
const SPCTL: &str = "/usr/sbin/spctl";

/// 解压新版本、存放旧版本备份的文件夹，在应用旁边。
const STAGING: &str = ".ScreenFerry-update";

pub struct MacUpdater {
    /// 当前进程所在的应用；不是从应用里运行时为空。
    bundle: Option<PathBuf>,
}

impl MacUpdater {
    pub fn new() -> Self {
        Self {
            bundle: current_bundle(),
        }
    }

    fn bundle(&self) -> Result<&Path> {
        self.bundle
            .as_deref()
            .ok_or(Error::Unsupported("没有从应用里运行"))
    }
}

impl Default for MacUpdater {
    fn default() -> Self {
        Self::new()
    }
}

impl Updater for MacUpdater {
    fn platform(&self) -> Option<&'static str> {
        Some("macos-universal")
    }

    fn can_install(&self) -> Result<()> {
        let bundle = self.bundle()?;
        if bundle.to_string_lossy().contains("/AppTranslocation/") {
            return Err(Error::Unsupported(
                "请先把 ScreenFerry 拖进“应用程序”文件夹",
            ));
        }
        let requirement = designated_requirement(bundle)?;
        if !requirement.contains("anchor apple generic") {
            return Err(Error::Unsupported("这份是临时签名的开发版本"));
        }
        let parent = bundle
            .parent()
            .ok_or(Error::Unsupported("找不到应用所在的文件夹"))?;
        let probe = parent.join(".ScreenFerry-write-test");
        if fs::write(&probe, b"").is_err() {
            return Err(Error::Unsupported("应用所在的文件夹没有写权限"));
        }
        let _ = fs::remove_file(&probe);
        Ok(())
    }

    fn proxy(&self) -> Option<String> {
        let output = Command::new(SCUTIL).arg("--proxy").output().ok()?;
        parse_scutil_proxy(&String::from_utf8_lossy(&output.stdout))
    }

    fn prepare(&self, package: &Path, version: &str) -> Result<Prepared> {
        self.can_install()?;
        let bundle = self.bundle()?;
        let parent = bundle
            .parent()
            .ok_or(Error::Unsupported("找不到应用所在的文件夹"))?;
        let staging = parent.join(STAGING);
        let _ = fs::remove_dir_all(&staging);
        fs::create_dir_all(&staging).map_err(|e| failed("建临时文件夹", e))?;

        let unpacked = run(
            DITTO,
            &[
                "-x".as_ref(),
                "-k".as_ref(),
                package.as_os_str(),
                staging.as_os_str(),
            ],
        );
        let name = bundle.file_name().unwrap_or(OsStr::new("ScreenFerry.app"));
        let new_app = staging.join(name);
        if unpacked.is_err() || !new_app.is_dir() {
            let _ = fs::remove_dir_all(&staging);
            return Err(Error::Failed("更新包里没有 ScreenFerry.app".into()));
        }

        let checked = verify(bundle, &new_app, version);
        if let Err(e) = checked {
            let _ = fs::remove_dir_all(&staging);
            return Err(e);
        }
        if let Err(e) = swap(bundle, &new_app) {
            let _ = fs::remove_dir_all(&staging);
            return Err(e);
        }
        // 交换之后，`new_app` 那个位置放的是旧版本。
        Ok(Prepared {
            target: bundle.to_owned(),
            backup: Some(new_app),
            restart_self: true,
        })
    }

    fn launch(&self, prepared: &Prepared) -> Result<()> {
        // -n：另开一份（旧进程还在，等新版本起来）；-g：不抢前台。
        run(
            OPEN,
            &[
                "-n".as_ref(),
                "-g".as_ref(),
                prepared.target.as_os_str(),
                "--args".as_ref(),
                "--autostart".as_ref(),
            ],
        )
        .map(drop)
    }

    fn confirm(&self, prepared: &Prepared) {
        if let Some(staging) = prepared.backup.as_deref().and_then(Path::parent) {
            let _ = fs::remove_dir_all(staging);
        }
    }

    fn rollback(&self, prepared: &Prepared) -> Result<()> {
        let backup = prepared
            .backup
            .as_deref()
            .ok_or(Error::Unsupported("没有旧版本的备份"))?;
        swap(&prepared.target, backup)?;
        self.confirm(prepared);
        Ok(())
    }
}

/// 新应用：签名满足本机这份的指定要求、通过 Gatekeeper（已公证）、版本号对得上。
fn verify(bundle: &Path, new_app: &Path, version: &str) -> Result<()> {
    let requirement = format!("={}", designated_requirement(bundle)?);
    run(
        CODESIGN,
        &[
            "--verify".as_ref(),
            "--deep".as_ref(),
            "--strict".as_ref(),
            "-R".as_ref(),
            requirement.as_ref(),
            new_app.as_os_str(),
        ],
    )
    .map_err(|_| Error::Failed("新版本的签名和这份对不上".into()))?;
    run(
        SPCTL,
        &[
            "--assess".as_ref(),
            "--type".as_ref(),
            "execute".as_ref(),
            new_app.as_os_str(),
        ],
    )
    .map_err(|_| Error::Failed("新版本没有通过 Gatekeeper 检查（可能没有公证）".into()))?;
    let plist = new_app.join("Contents/Info.plist");
    let found = run(
        PLUTIL,
        &[
            "-extract".as_ref(),
            "CFBundleShortVersionString".as_ref(),
            "raw".as_ref(),
            "-o".as_ref(),
            "-".as_ref(),
            plist.as_os_str(),
        ],
    )?;
    if found.trim() != version {
        return Err(Error::Failed(format!(
            "更新包里的版本是 {}，不是 {version}",
            found.trim()
        )));
    }
    Ok(())
}

/// 应用签名里的指定要求，例如 `identifier "…" and anchor apple generic and … leaf[subject.OU] = …`。
fn designated_requirement(bundle: &Path) -> Result<String> {
    let output = Command::new(CODESIGN)
        .args(["-d", "-r-"])
        .arg(bundle)
        .output()
        .map_err(|e| failed("运行 codesign", e))?;
    let text = [output.stdout, output.stderr].concat();
    String::from_utf8_lossy(&text)
        .lines()
        .find_map(|line| line.strip_prefix("designated => "))
        .map(str::to_owned)
        .ok_or(Error::Unsupported("应用没有签名"))
}

/// 一步交换两个路径上的文件夹。两边必须在同一个磁盘上。
fn swap(a: &Path, b: &Path) -> Result<()> {
    let a =
        CString::new(a.as_os_str().as_bytes()).map_err(|_| Error::Unsupported("路径里有空字符"))?;
    let b =
        CString::new(b.as_os_str().as_bytes()).map_err(|_| Error::Unsupported("路径里有空字符"))?;
    // SAFETY: 两个参数都是以 0 结尾的有效 C 字符串，调用期间一直有效。
    let rc = unsafe { libc::renamex_np(a.as_ptr(), b.as_ptr(), libc::RENAME_SWAP) };
    if rc == 0 {
        Ok(())
    } else {
        Err(Error::Os {
            call: "renamex_np",
            detail: std::io::Error::last_os_error().to_string(),
        })
    }
}

/// 运行一个系统工具，失败时带上它的输出。返回标准输出。
fn run(program: &str, args: &[&OsStr]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| failed(program, e))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(Error::Failed(format!(
            "{program}：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

fn failed(what: &str, e: std::io::Error) -> Error {
    Error::Failed(format!("{what}：{e}"))
}

/// 当前进程所在的应用：可执行文件在 `X.app/Contents/MacOS/` 下。
fn current_bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    let bundle = exe.parent()?.parent()?.parent()?;
    (bundle.extension()? == "app").then(|| bundle.to_owned())
}

/// 从 `scutil --proxy` 的输出里取 HTTPS 代理（没有时取 HTTP 代理），写成 `http://主机:端口`。
pub fn parse_scutil_proxy(text: &str) -> Option<String> {
    let value = |key: &str| {
        text.lines().find_map(|line| {
            let (k, v) = line.trim().split_once(" : ")?;
            (k == key).then(|| v.trim().to_owned())
        })
    };
    ["HTTPS", "HTTP"].into_iter().find_map(|kind| {
        if value(&format!("{kind}Enable")).as_deref() != Some("1") {
            return None;
        }
        let host = value(&format!("{kind}Proxy")).filter(|h| !h.is_empty())?;
        let port = value(&format!("{kind}Port"))?;
        Some(format!("http://{host}:{port}"))
    })
}
