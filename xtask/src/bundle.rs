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
//! Windows 先组装 `target/bundle/windows/ScreenFerry/` 目录，再用 Inno Setup 打成安装包
//! `target/bundle/windows/ScreenFerry-<版本>-windows-x64-setup.exe`（`packaging/windows/screenferry.iss`，ADR 0011）：
//!
//! ```text
//! ScreenFerry\
//! ├─ screenferry.exe                后台，C 运行库静态链接；图标、版本信息、程序清单见 packaging/windows/
//! └─ settings\                      Flutter 设置界面：screenferry-settings.exe、flutter_windows.dll、data\，
//!                                    以及它需要的 Visual C++ 运行库（msvcp140.dll 等三个）
//! ```
//!
//! 需要 Visual Studio（C++ 工具，附带运行库）和 Inno Setup 7.1 或更高版本（自带简体中文）；
//! ISCC.exe 不在默认位置时用环境变量 `ISCC` 指定。

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

/// 设置界面（Flutter）依赖的 Visual C++ 运行库，放在它旁边，没装过运行库的电脑也能打开。
const VC_RUNTIME: [&str; 3] = ["msvcp140.dll", "vcruntime140.dll", "vcruntime140_1.dll"];

fn windows(root: &Path) -> Result {
    let version = workspace_version(root)?;
    // 先找齐工具，免得编译完才发现缺东西。
    let iscc = find_iscc()?;
    let runtime = vc_runtime_dir()?;

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
    for dll in VC_RUNTIME {
        copy(&runtime.join(dll), &out.join("settings").join(dll))?;
    }

    step("打安装包（Inno Setup）");
    let installers = root.join("target/bundle/windows");
    let setup = installers.join(format!("ScreenFerry-{version}-windows-x64-setup.exe"));
    if setup.exists() {
        fs::remove_file(&setup).map_err(|e| io(&setup, e))?;
    }
    let status = Command::new(&iscc)
        .arg("--quiet-progress")
        .arg(format!("/DAppVersion={version}"))
        .arg(format!("/DSourceDir={}", out.display()))
        .arg(format!("--output-dir={}", installers.display()))
        .arg(root.join("packaging/windows/screenferry.iss"))
        .status()
        .map_err(|e| format!("无法运行 {}：{e}", iscc.display()))?;
    if !status.success() || !setup.exists() {
        return Err("Inno Setup 没有打出安装包".into());
    }

    println!("\n✓ {}", setup.display());
    println!(
        "  双击安装：装进 Program Files，开始菜单里有 ScreenFerry，可以在“设置 → 应用”里卸载。"
    );
    println!(
        "  不安装直接试：双击 {}",
        out.join("screenferry.exe").display()
    );
    Ok(())
}

/// 找 Inno Setup 的命令行编译器：环境变量 `ISCC`，或者默认的安装位置。
/// 需要 7.1 或更高版本：从这一版起自带简体中文。
fn find_iscc() -> Result<PathBuf> {
    let mut candidates: Vec<PathBuf> = std::env::var_os("ISCC")
        .map(PathBuf::from)
        .into_iter()
        .collect();
    for (var, sub) in [
        ("ProgramFiles", ""),
        ("ProgramFiles(x86)", ""),
        ("LOCALAPPDATA", "Programs"),
    ] {
        if let Some(dir) = std::env::var_os(var) {
            candidates.push(
                PathBuf::from(dir)
                    .join(sub)
                    .join("Inno Setup 7")
                    .join("ISCC.exe"),
            );
        }
    }
    let iscc = candidates.into_iter().find(|p| p.is_file()).ok_or(
        "找不到 Inno Setup 7 的 ISCC.exe：从 https://jrsoftware.org/isdl.php 安装 7.1 或更高版本，\
         或者用环境变量 ISCC 指定",
    )?;
    let chinese = iscc
        .with_file_name("Languages")
        .join("ChineseSimplified.isl");
    if !chinese.is_file() {
        return Err(format!(
            "{} 太旧：需要 Inno Setup 7.1 或更高版本（自带简体中文）",
            iscc.display()
        ));
    }
    Ok(iscc)
}

/// Visual C++ 运行库所在的目录（`…\VC\Redist\MSVC\<版本>\x64\Microsoft.VC14x.CRT`）。
/// 开发者命令行里有 `VCToolsRedistDir`；否则用 vswhere 找装了 C++ 工具的 Visual Studio，取最新的一版。
fn vc_runtime_dir() -> Result<PathBuf> {
    let redist = match std::env::var_os("VCToolsRedistDir") {
        Some(dir) => PathBuf::from(dir),
        None => {
            let program_files =
                std::env::var_os("ProgramFiles(x86)").ok_or("找不到 Program Files (x86)")?;
            let vswhere =
                PathBuf::from(program_files).join(r"Microsoft Visual Studio\Installer\vswhere.exe");
            let output = Command::new(&vswhere)
                .args(["-latest", "-products", "*", "-requires"])
                .arg("Microsoft.VisualStudio.Component.VC.Tools.x86.x64")
                .args(["-property", "installationPath"])
                .output()
                .map_err(|e| {
                    format!("无法运行 vswhere（需要装有 C++ 工具的 Visual Studio）：{e}")
                })?;
            let install = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            if install.is_empty() {
                return Err("找不到装有 C++ 工具的 Visual Studio".into());
            }
            let msvc = PathBuf::from(install).join(r"VC\Redist\MSVC");
            subdirs(&msvc)?
                .into_iter()
                .filter_map(|dir| Some((version_key(&dir)?, dir)))
                .max()
                .map(|(_, dir)| dir)
                .ok_or_else(|| format!("{} 里没有运行库", msvc.display()))?
        }
    };
    let x64 = redist.join("x64");
    subdirs(&x64)?
        .into_iter()
        .filter(|dir| {
            dir.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("Microsoft.VC") && n.ends_with(".CRT"))
        })
        .max()
        .filter(|dir| VC_RUNTIME.iter().all(|dll| dir.join(dll).is_file()))
        .ok_or_else(|| format!("{} 里没有完整的 Visual C++ 运行库", x64.display()))
}

/// 目录下的子目录。
fn subdirs(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| io(dir, e))? {
        let path = entry.map_err(|e| io(dir, e))?.path();
        if path.is_dir() {
            found.push(path);
        }
    }
    Ok(found)
}

/// 形如 `14.44.35112` 的目录名按数字比较；不是版本号的（例如 `v143`）返回 `None`。
fn version_key(dir: &Path) -> Option<Vec<u32>> {
    dir.file_name()?
        .to_str()?
        .split('.')
        .map(|part| part.parse().ok())
        .collect()
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
    let mut command = Command::new(cargo);
    command.args(args).current_dir(root);
    // 诊断文件里写上是哪次提交打的包。
    if let Some(commit) = commit(root) {
        command.env("SCREENFERRY_COMMIT", commit);
    }
    let status = command
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

/// 当前提交号的前 7 位；不在 git 仓库里时为 `None`。
fn commit(root: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short=7", "HEAD"])
        .current_dir(root)
        .output()
        .ok()?;
    let commit = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (output.status.success() && !commit.is_empty()).then_some(commit)
}
