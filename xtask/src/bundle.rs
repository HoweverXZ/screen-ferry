//! `cargo xtask bundle`：把后台与设置界面打成一个应用。
//!
//! macOS 产物是 `target/bundle/macos/ScreenFerry.app`：
//!
//! ```text
//! ScreenFerry.app/Contents/
//! ├─ Info.plist                      packaging/macos/Info.plist，填入版本号
//! ├─ MacOS/screenferry               后台（主程序，不在程序坞里显示）
//! ├─ Resources/AppIcon.icns          取自设置界面
//! └─ Helpers/ScreenFerry Settings.app  Flutter 设置界面
//! ```
//!
//! 签名用临时签名（ad-hoc），只适合本机使用；正式发布时换成开发者证书并公证。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

type Result<T = ()> = std::result::Result<T, String>;

pub(crate) fn run() -> Result {
    if std::env::consts::OS != "macos" {
        return Err("目前只能在 macOS 上打包 macOS 版；Windows 版见第四阶段".into());
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("找不到仓库根目录")?
        .to_path_buf();
    let version = workspace_version(&root)?;

    step("编译后台（release）");
    cargo(&root, &["build", "--release", "-p", "ferry-daemon"])?;

    step("编译设置界面（flutter build macos --release）");
    let app_dir = root.join("app");
    run_in(&app_dir, "flutter", &["build", "macos", "--release"])?;
    let settings = app_dir.join("build/macos/Build/Products/Release/ScreenFerry.app");

    step("组装 ScreenFerry.app");
    let bundle = root.join("target/bundle/macos/ScreenFerry.app");
    if bundle.exists() {
        fs::remove_dir_all(&bundle).map_err(|e| io(&bundle, e))?;
    }
    let contents = bundle.join("Contents");
    for dir in ["MacOS", "Resources", "Helpers"] {
        fs::create_dir_all(contents.join(dir)).map_err(|e| io(&contents, e))?;
    }

    let plist = fs::read_to_string(root.join("packaging/macos/Info.plist"))
        .map_err(|e| io(&root.join("packaging/macos/Info.plist"), e))?
        .replace("{{VERSION}}", &version);
    write(&contents.join("Info.plist"), plist.as_bytes())?;

    copy(
        &root.join("target/release/screenferry"),
        &contents.join("MacOS/screenferry"),
    )?;
    copy(
        &settings.join("Contents/Resources/AppIcon.icns"),
        &contents.join("Resources/AppIcon.icns"),
    )?;
    // ditto 保留 Flutter 框架里的符号链接和已有的签名。
    run_in(
        &root,
        "ditto",
        &[
            settings.to_str().ok_or("路径不是 UTF-8")?,
            contents
                .join("Helpers/ScreenFerry Settings.app")
                .to_str()
                .ok_or("路径不是 UTF-8")?,
        ],
    )?;

    step("签名（临时签名，仅限本机）");
    // Flutter 产物里的 App.framework 签名后又被改过，严格校验通不过；先整体重签内层的设置界面。
    let helper = contents.join("Helpers/ScreenFerry Settings.app");
    run_in(
        &root,
        "codesign",
        &[
            "--force",
            "--deep",
            "--sign",
            "-",
            helper.to_str().ok_or("路径不是 UTF-8")?,
        ],
    )?;
    let entitlements = root.join("packaging/macos/entitlements.plist");
    run_in(
        &root,
        "codesign",
        &[
            "--force",
            "--sign",
            "-",
            "--entitlements",
            entitlements.to_str().ok_or("路径不是 UTF-8")?,
            bundle.to_str().ok_or("路径不是 UTF-8")?,
        ],
    )?;

    run_in(
        &root,
        "codesign",
        &[
            "--verify",
            "--deep",
            "--strict",
            bundle.to_str().ok_or("路径不是 UTF-8")?,
        ],
    )?;

    println!("\n✓ {}", bundle.display());
    println!("  打开：open \"{}\"", bundle.display());
    println!("  安装：拖到“应用程序”文件夹。首次运行需要在系统设置里允许它控制键盘和鼠标；");
    println!("        临时签名的应用每次重新打包后，可能要在“辅助功能”里关掉再打开一次。");
    Ok(())
}

fn workspace_version(root: &Path) -> Result<String> {
    let text = fs::read_to_string(root.join("Cargo.toml")).map_err(|e| e.to_string())?;
    let manifest: toml::Table = text.parse().map_err(|e| format!("Cargo.toml：{e}"))?;
    manifest
        .get("workspace")
        .and_then(|w| w.get("package"))
        .and_then(|p| p.get("version"))
        .and_then(|v| v.as_str())
        .map(str::to_owned)
        .ok_or_else(|| "Cargo.toml 中没有 [workspace.package] version".into())
}

fn step(what: &str) {
    println!("==> {what}");
}

fn cargo(root: &Path, args: &[&str]) -> Result {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(cargo)
        .args(args)
        .current_dir(root)
        .status()
        .map_err(|e| format!("无法运行 cargo：{e}"))?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| format!("cargo {} 失败", args.join(" ")))
}

fn run_in(dir: &Path, program: &str, args: &[&str]) -> Result {
    let status = Command::new(program)
        .args(args)
        .current_dir(dir)
        .status()
        .map_err(|e| format!("无法运行 {program}：{e}"))?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| format!("{program} {} 失败", args.join(" ")))
}

fn copy(from: &Path, to: &Path) -> Result {
    fs::copy(from, to).map(drop).map_err(|e| io(from, e))
}

fn write(path: &Path, data: &[u8]) -> Result {
    fs::write(path, data).map_err(|e| io(path, e))
}

fn io(path: &Path, e: std::io::Error) -> String {
    format!("{}：{e}", PathBuf::from(path).display())
}
