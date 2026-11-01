//! `cargo xtask bundle`：把后台与设置界面打成一个应用。在哪个系统上运行就打哪个系统的包。
//!
//! macOS 产物是 `target/bundle/macos/ScreenFerry.app`：
//!
//! ```text
//! ScreenFerry.app/Contents/
//! ├─ Info.plist                      packaging/macos/Info.plist，填入版本号
//! ├─ MacOS/screenferry               后台（主程序，不在程序坞里显示）
//! ├─ Resources/AppIcon.icns          取自设置界面
//! └─ Helpers/ScreenFerry.app      Flutter 设置界面（和外层同名：程序坞里显示的就是软件名）
//! ```
//!
//! 签名：环境变量 `SCREENFERRY_SIGN_IDENTITY` 指定的证书优先，其次钥匙串里第一张
//! “Developer ID Application”证书，都没有时用临时签名（ad-hoc）。用固定的开发者证书签名后，
//! 系统按“团队 + bundle id”认这个应用，重新打包也不会丢掉已经授予的辅助功能权限；
//! 临时签名每次打包都像一个新应用。签名开启 hardened runtime，为以后公证做准备。
//!
//! Windows 产物是 `target/bundle/windows/ScreenFerry/` 目录：
//!
//! ```text
//! ScreenFerry\
//! ├─ screenferry.exe                后台
//! └─ settings\                      Flutter 设置界面（screenferry_settings.exe 及其运行库）
//! ```

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

type Result<T = ()> = std::result::Result<T, String>;

/// 设置界面在应用里的位置。和外层同名，程序坞、切换器里显示的都是软件名。
const HELPER: &str = "Helpers/ScreenFerry.app";
/// 系统登记应用的工具，不在 PATH 里。
const LSREGISTER: &str = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister";

pub(crate) fn run() -> Result {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("找不到仓库根目录")?
        .to_path_buf();
    match std::env::consts::OS {
        "macos" => macos(&root),
        "windows" => windows(&root),
        other => Err(format!("还不支持在 {other} 上打包")),
    }
}

fn windows(root: &Path) -> Result {
    step("编译后台（release）");
    cargo(root, &["build", "--release", "-p", "ferry-daemon"])?;

    step("编译设置界面（flutter build windows --release）");
    let app_dir = root.join("app");
    flutter(&app_dir, &["build", "windows", "--release"])?;
    let built = app_dir.join("build/windows/x64/runner/Release");

    step("组装 ScreenFerry 目录");
    let out = root.join("target/bundle/windows/ScreenFerry");
    if out.exists() {
        fs::remove_dir_all(&out).map_err(|e| io(&out, e))?;
    }
    fs::create_dir_all(&out).map_err(|e| io(&out, e))?;
    copy(
        &root.join("target/release/screenferry.exe"),
        &out.join("screenferry.exe"),
    )?;
    copy_dir(&built, &out.join("settings"))?;

    println!("\n✓ {}", out.display());
    println!("  运行：双击 screenferry.exe（会打开设置界面，任务栏右下角出现图标）");
    println!(
        "  整个 ScreenFerry 目录可以放到任意位置，例如 %LOCALAPPDATA%\\Programs\\ScreenFerry。"
    );
    Ok(())
}

fn macos(root: &Path) -> Result {
    let root = root.to_path_buf();
    let version = workspace_version(&root)?;

    step("编译后台（release）");
    cargo(&root, &["build", "--release", "-p", "ferry-daemon"])?;

    step("编译设置界面（flutter build macos --release）");
    let app_dir = root.join("app");
    flutter(&app_dir, &["build", "macos", "--release"])?;
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
            contents.join(HELPER).to_str().ok_or("路径不是 UTF-8")?,
        ],
    )?;

    let identity = signing_identity();
    step(&format!(
        "签名（{}）",
        if identity == "-" {
            "临时签名，仅限本机"
        } else {
            identity.as_str()
        }
    ));
    let runtime: &[&str] = if identity == "-" {
        &[]
    } else {
        &["--options", "runtime"]
    };
    // Flutter 产物里的 App.framework 签名后又被改过，严格校验通不过；先整体重签内层的设置界面。
    let helper = contents.join(HELPER);
    let mut args = vec!["--force", "--deep", "--sign", identity.as_str()];
    args.extend_from_slice(runtime);
    args.push(helper.to_str().ok_or("路径不是 UTF-8")?);
    run_in(&root, "codesign", &args)?;

    let entitlements = root.join("packaging/macos/entitlements.plist");
    let mut args = vec!["--force", "--sign", identity.as_str()];
    args.extend_from_slice(runtime);
    args.extend_from_slice(&[
        "--entitlements",
        entitlements.to_str().ok_or("路径不是 UTF-8")?,
        bundle.to_str().ok_or("路径不是 UTF-8")?,
    ]);
    run_in(&root, "codesign", &args)?;

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

    refresh_icons(&root, &[&bundle, &helper]);

    println!("\n✓ {}", bundle.display());
    println!("  打开：open \"{}\"", bundle.display());
    println!("  安装：拖到“应用程序”文件夹。首次运行需要在系统设置里允许它控制键盘和鼠标。");
    if identity == "-" {
        println!("        临时签名的应用每次重新打包后，可能要在“辅助功能”里关掉再打开一次。");
    }
    Ok(())
}

/// 访达和程序坞按路径缓存应用图标，原地重新打包后还显示旧图标。碰一下修改时间、重新登记，
/// 让它们重新读。做不到不影响打包。
fn refresh_icons(root: &Path, apps: &[&Path]) {
    for app in apps {
        let Some(path) = app.to_str() else { continue };
        if run_in(root, "touch", &[path]).is_err()
            || run_in(root, LSREGISTER, &["-f", path]).is_err()
        {
            eprintln!("提示：没能刷新 {path} 的图标缓存，可能要重新登录才显示新图标");
        }
    }
}

/// 选签名证书：环境变量指定的优先，其次钥匙串里第一张 Developer ID Application 证书，
/// 都没有时用临时签名 `-`。
fn signing_identity() -> String {
    if let Ok(identity) = std::env::var("SCREENFERRY_SIGN_IDENTITY")
        && !identity.trim().is_empty()
    {
        return identity;
    }
    let output = Command::new("security")
        .args(["find-identity", "-v", "-p", "codesigning"])
        .output();
    let Ok(output) = output else {
        return "-".into();
    };
    // 每行形如：  1) <40 位哈希> "Developer ID Application: 名字 (团队)"
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| line.contains("\"Developer ID Application:"))
        .find_map(|line| line.split('"').nth(1).map(str::to_owned))
        .unwrap_or_else(|| "-".into())
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

/// Windows 上的 flutter 是批处理文件，要经 cmd 运行。
fn flutter(dir: &Path, args: &[&str]) -> Result {
    if std::env::consts::OS == "windows" {
        let mut all = vec!["/C", "flutter"];
        all.extend_from_slice(args);
        run_in(dir, "cmd", &all)
    } else {
        run_in(dir, "flutter", args)
    }
}

/// 递归复制目录。
fn copy_dir(from: &Path, to: &Path) -> Result {
    fs::create_dir_all(to).map_err(|e| io(to, e))?;
    for entry in fs::read_dir(from).map_err(|e| io(from, e))? {
        let entry = entry.map_err(|e| io(from, e))?;
        let path = entry.path();
        let target = to.join(entry.file_name());
        if path.is_dir() {
            copy_dir(&path, &target)?;
        } else {
            copy(&path, &target)?;
        }
    }
    Ok(())
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
