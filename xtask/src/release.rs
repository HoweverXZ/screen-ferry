//! 发布（ADR 0014）：
//!
//! - `release <版本>`：改版本号（Cargo.toml、app/pubspec.yaml、Cargo.lock），整理 CHANGELOG，
//!   检查 `docs/releases/<版本>.md` 里的更新说明，然后提交并打 tag。推送由人来做，推上 tag 后
//!   CI（`.github/workflows/release.yml`）构建两个系统的安装包，存成 GitHub 上的草稿。
//! - `manifest`：CI 用。按目录里的更新包写 `update.json`：版本、发布和过期时间、各平台的地址、
//!   大小和 SHA-256、线协议版本、更新说明。
//! - `sign <文件>`：CI 用。用发布私钥给文件做 minisign 签名，写出 `<文件>.minisig`，再用程序内置的
//!   公钥校验一遍，确认签名的密钥是程序认的。私钥和口令来自环境变量
//!   `SCREENFERRY_UPDATE_KEY`（私钥文件的内容）、`SCREENFERRY_UPDATE_KEY_PASSWORD`。
//! - `update-keygen`：生成两把发布密钥（日常用、备用），私钥加口令后写到仓库以外的目录，
//!   两把公钥写进 `crates/ferry-update/src/keys.rs`。只需运行一次。

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use ferry_update::{Asset, Manifest, Notes, RELEASE_KEYS, Release, SCHEMA};
use ring::digest::{Context, SHA256};

type Result<T = ()> = std::result::Result<T, String>;

/// 清单多久过期：过期后程序不再说“已是最新”。发布间隔超过它时要重新签一份清单。
const MANIFEST_LIFETIME: i64 = 180 * 86_400;

/// 各平台更新包的文件名结尾，对应清单里的平台名。
const PLATFORMS: [(&str, &str); 2] = [
    ("macos-universal", "-macos.zip"),
    ("windows-x86_64", "-windows-x64-setup.exe"),
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask 在仓库根目录下")
        .to_path_buf()
}

// ── release ───────────────────────────────────

pub(crate) fn release(args: &[String]) -> Result {
    let [version] = args else {
        return Err("用法：cargo xtask release <版本>，例如 0.2.0".into());
    };
    let parsed =
        ferry_update::Version::parse(version).map_err(|e| format!("版本号 {version}：{e}"))?;
    let root = root();
    let notes_path = root.join(format!("docs/releases/{version}.md"));
    if !notes_path.exists() {
        if let Some(dir) = notes_path.parent() {
            fs::create_dir_all(dir).map_err(|e| io(dir, e))?;
        }
        let template =
            format!("# ScreenFerry {version}\n\n## 简体中文\n\n- \n\n## English\n\n- \n");
        fs::write(&notes_path, template).map_err(|e| io(&notes_path, e))?;
        return Err(format!(
            "先在 {} 里写好中英文的更新说明（每条一行，以“- ”开头），再运行一次",
            notes_path.display()
        ));
    }
    let notes = read_notes(&notes_path)?;
    if notes.zh.is_empty() || notes.en.is_empty() {
        return Err(format!(
            "{} 里的中文或英文更新说明是空的",
            notes_path.display()
        ));
    }

    let cargo_toml = root.join("Cargo.toml");
    let text = fs::read_to_string(&cargo_toml).map_err(|e| io(&cargo_toml, e))?;
    let current = workspace_version(&text)?;
    if ferry_update::Version::parse(&current).is_ok_and(|c| c >= parsed) {
        return Err(format!("{version} 不比现在的 {current} 新"));
    }
    let text = text.replacen(
        &format!("version = \"{current}\""),
        &format!("version = \"{version}\""),
        1,
    );
    fs::write(&cargo_toml, text).map_err(|e| io(&cargo_toml, e))?;

    let pubspec = root.join("app/pubspec.yaml");
    let text = fs::read_to_string(&pubspec).map_err(|e| io(&pubspec, e))?;
    let mut build = 1;
    let text: String = text
        .lines()
        .map(|line| match line.strip_prefix("version: ") {
            Some(old) => {
                build = old
                    .split('+')
                    .nth(1)
                    .and_then(|b| b.trim().parse::<u32>().ok())
                    .map_or(1, |b| b + 1);
                format!("version: {version}+{build}")
            }
            None => line.to_owned(),
        })
        .map(|line| line + "\n")
        .collect();
    fs::write(&pubspec, text).map_err(|e| io(&pubspec, e))?;

    let changelog = root.join("CHANGELOG.md");
    let text = fs::read_to_string(&changelog).map_err(|e| io(&changelog, e))?;
    if !text.contains("## [Unreleased]") {
        return Err("CHANGELOG.md 里没有 ## [Unreleased]".into());
    }
    let text = text.replacen(
        "## [Unreleased]",
        &format!("## [Unreleased]\n\n## [{version}] - {}", today()),
        1,
    );
    fs::write(&changelog, text).map_err(|e| io(&changelog, e))?;

    run(&root, "cargo", &["update", "--workspace", "--offline"])?;
    let notes_rel = format!("docs/releases/{version}.md");
    run(
        &root,
        "git",
        &[
            "add",
            "Cargo.toml",
            "Cargo.lock",
            "app/pubspec.yaml",
            "CHANGELOG.md",
            &notes_rel,
        ],
    )?;
    run(
        &root,
        "git",
        &["commit", "-m", &format!("release: {version}")],
    )?;
    run(
        &root,
        "git",
        &[
            "tag",
            "-a",
            &format!("v{version}"),
            "-m",
            &format!("ScreenFerry {version}"),
        ],
    )?;
    println!("\n✓ 已提交并打好 tag v{version}（设置界面的构建号 {build}）");
    println!("  推送后 CI 会构建安装包、存成 GitHub 上的草稿：");
    println!("    git push && git push origin v{version}");
    Ok(())
}

/// `docs/releases/<版本>.md`：`## 简体中文`、`## English` 两节下以“- ”开头的行。
fn read_notes(path: &Path) -> Result<Notes> {
    let text = fs::read_to_string(path).map_err(|e| io(path, e))?;
    let mut notes = Notes::default();
    let mut section = None;
    for line in text.lines() {
        let line = line.trim();
        if let Some(title) = line.strip_prefix("## ") {
            section = match title.trim() {
                "简体中文" => Some(true),
                "English" => Some(false),
                _ => None,
            };
            continue;
        }
        let Some(item) = line
            .strip_prefix("- ")
            .map(str::trim)
            .filter(|i| !i.is_empty())
        else {
            continue;
        };
        let target = match section {
            Some(true) => &mut notes.zh,
            Some(false) => &mut notes.en,
            None => continue,
        };
        if !target.is_empty() {
            target.push('\n');
        }
        target.push_str(item);
    }
    Ok(notes)
}

// ── manifest ──────────────────────────────────

/// `manifest --version 0.2.0 --dir dist --base-url https://…/releases/download/v0.2.0 --out dist/update.json`
pub(crate) fn manifest(args: &[String]) -> Result {
    let opt = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
            .ok_or_else(|| format!("缺少 {name}"))
    };
    let version = opt("--version")?;
    let dir = PathBuf::from(opt("--dir")?);
    let base = opt("--base-url")?.trim_end_matches('/').to_owned();
    let out = PathBuf::from(opt("--out")?);
    let root = root();
    let notes = read_notes(&root.join(format!("docs/releases/{version}.md")))?;

    let mut assets = std::collections::BTreeMap::new();
    for entry in fs::read_dir(&dir).map_err(|e| io(&dir, e))? {
        let path = entry.map_err(|e| io(&dir, e))?.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some((platform, _)) = PLATFORMS.iter().find(|(_, suffix)| name.ends_with(suffix))
        else {
            continue;
        };
        if !name.contains(&format!("-{version}-")) {
            return Err(format!("{name} 的版本号不是 {version}"));
        }
        let (size, sha256) = hash(&path)?;
        println!("  {platform}: {name}（{size} 字节）");
        assets.insert(
            (*platform).to_owned(),
            Asset {
                url: format!("{base}/{name}"),
                size,
                sha256,
            },
        );
    }
    if assets.is_empty() {
        return Err(format!("{} 里没有更新包", dir.display()));
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs() as i64;
    let manifest = Manifest {
        schema: SCHEMA,
        version,
        published: Manifest::format_time(now),
        expires: Manifest::format_time(now + MANIFEST_LIFETIME),
        protocol: ferry_proto::PROTOCOL_VERSION,
        notes,
        assets,
    };
    Release::from_manifest(manifest.clone()).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    fs::write(&out, json + "\n").map_err(|e| io(&out, e))?;
    println!("✓ {}", out.display());
    Ok(())
}

fn hash(path: &Path) -> Result<(u64, String)> {
    let data = fs::read(path).map_err(|e| io(path, e))?;
    let mut ctx = Context::new(&SHA256);
    ctx.update(&data);
    let hex = ctx
        .finish()
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    Ok((data.len() as u64, hex))
}

// ── sign ──────────────────────────────────────

pub(crate) fn sign(args: &[String]) -> Result {
    let [file] = args else {
        return Err("用法：cargo xtask sign <文件>".into());
    };
    let key = std::env::var("SCREENFERRY_UPDATE_KEY")
        .ok()
        .filter(|k| !k.trim().is_empty())
        .ok_or("没有发布私钥：环境变量 SCREENFERRY_UPDATE_KEY")?;
    let password = std::env::var("SCREENFERRY_UPDATE_KEY_PASSWORD").ok();
    let secret = minisign::SecretKeyBox::from_string(&key)
        .and_then(|b| b.into_secret_key(password))
        .map_err(|e| format!("读不了发布私钥（口令对吗？）：{e}"))?;
    let path = PathBuf::from(file);
    let data = fs::read(&path).map_err(|e| io(&path, e))?;
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let signature = minisign::sign(None, &secret, Cursor::new(&data), Some(name), None)
        .map_err(|e| format!("签名失败：{e}"))?
        .into_string();
    let out = PathBuf::from(format!("{file}.minisig"));
    fs::write(&out, &signature).map_err(|e| io(&out, e))?;

    // 用程序内置的公钥校验一遍：签名的密钥不是程序认的，发出去也没人能更新。
    Release::verify(&data, &signature, RELEASE_KEYS).map_err(|e| {
        format!("签好了，但程序内置的公钥校验不过（{e}）：发布私钥和 keys.rs 里的公钥对不上")
    })?;
    println!("✓ {}", out.display());
    Ok(())
}

// ── update-keygen ─────────────────────────────

pub(crate) fn keygen(args: &[String]) -> Result {
    let out = match args {
        [] => home()?.join("ScreenFerry-release-keys"),
        [flag, dir] if flag == "--out" => PathBuf::from(dir),
        _ => return Err("用法：cargo xtask update-keygen [--out <目录>]".into()),
    };
    let root = root();
    if out.starts_with(&root) {
        return Err("私钥不能放在仓库里，换一个目录".into());
    }
    if !RELEASE_KEYS.is_empty() {
        return Err("keys.rs 里已经有发布公钥了。轮换密钥见 ADR 0014，不要直接覆盖".into());
    }
    fs::create_dir_all(&out).map_err(|e| io(&out, e))?;
    let mut publics = Vec::new();
    for (name, label) in [("release", "日常发布用"), ("backup", "备用，私钥离线保存")]
    {
        let key_path = out.join(format!("screenferry-{name}.key"));
        if key_path.exists() {
            return Err(format!("{} 已经存在，不覆盖", key_path.display()));
        }
        println!("\n为“{label}”的私钥设一个口令（输入时不显示）：");
        let pair = minisign::KeyPair::generate_encrypted_keypair(None)
            .map_err(|e| format!("生成密钥失败：{e}"))?;
        let secret = pair
            .sk
            .to_box(Some(&format!("ScreenFerry {label}")))
            .map_err(|e| e.to_string())?
            .into_string();
        write_private(&key_path, &secret)?;
        let public = pair.pk.to_base64();
        let pub_path = out.join(format!("screenferry-{name}.pub"));
        fs::write(&pub_path, format!("{public}\n")).map_err(|e| io(&pub_path, e))?;
        publics.push((public, label));
    }

    let keys = root.join("crates/ferry-update/src/keys.rs");
    let text = fs::read_to_string(&keys).map_err(|e| io(&keys, e))?;
    let (start, end) = ("// update-keygen：开始", "// update-keygen：结束");
    let (Some(a), Some(b)) = (text.find(start), text.find(end)) else {
        return Err("keys.rs 里找不到 update-keygen 的标记".into());
    };
    let lines: String = publics
        .iter()
        .map(|(key, label)| format!("    \"{key}\", // {label}，生成于 {}\n", today()))
        .collect();
    let text = format!("{}{start}\n{lines}    {}", &text[..a], &text[b..]);
    fs::write(&keys, text).map_err(|e| io(&keys, e))?;

    println!(
        "\n✓ 私钥在 {}，两把公钥已写进 {}",
        out.display(),
        keys.display()
    );
    println!("  接下来：");
    println!("  1. 在 GitHub 仓库的 Settings → Secrets and variables → Actions 里新建：");
    println!("       SCREENFERRY_UPDATE_KEY           screenferry-release.key 的全部内容");
    println!("       SCREENFERRY_UPDATE_KEY_PASSWORD  它的口令");
    println!("  2. 把 screenferry-backup.key 和它的口令存进密码管理器，然后从这台电脑上删掉。");
    println!("  3. 提交 keys.rs 的改动。");
    Ok(())
}

/// 写私钥文件：只有自己能读。
fn write_private(path: &Path, text: &str) -> Result {
    fs::write(path, text).map_err(|e| io(path, e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|e| io(path, e))?;
    }
    Ok(())
}

fn home() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or_else(|| "找不到用户目录".into())
}

// ── 小工具 ────────────────────────────────────

fn workspace_version(text: &str) -> Result<String> {
    let manifest: toml::Table = text.parse().map_err(|e| format!("Cargo.toml：{e}"))?;
    manifest
        .get("workspace")
        .and_then(|w| w.get("package"))
        .and_then(|p| p.get("version"))
        .and_then(|v| v.as_str())
        .map(str::to_owned)
        .ok_or_else(|| "Cargo.toml 中没有 [workspace.package] version".into())
}

/// 今天的日期（UTC），例如 `2026-11-02`。
fn today() -> String {
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() / 86_400) as i64;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

fn run(dir: &Path, program: &str, args: &[&str]) -> Result {
    let status = Command::new(program)
        .args(args)
        .current_dir(dir)
        .status()
        .map_err(|e| format!("无法运行 {program}：{e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} {} 失败", args.join(" ")))
    }
}

fn io(path: &Path, e: std::io::Error) -> String {
    format!("{}：{e}", path.display())
}
