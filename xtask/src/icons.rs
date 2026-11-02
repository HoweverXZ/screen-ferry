//! `cargo xtask icons`：把 `assets/icons/` 下的 SVG 源文件导出成各处要用的位图。需要 `rsvg-convert`
//! （macOS 上 `brew install librsvg`）。
//!
//! ```text
//! assets/icons/
//! ├─ tray-connected@2x.png / tray-disconnected@2x.png         macOS 菜单栏模板图，36×36
//! ├─ tray-win-connected.png / tray-win-disconnected.png        Windows 任务栏，32×32
//! └─ app-icon-windows.ico                                      Windows 应用图标，16–256：后台程序与安装包用
//! assets/dmg/background.png、background@2x.png                Mac 安装盘的窗口背景，660×400 和 1320×800（144 ppi）
//! app/macos/Runner/Assets.xcassets/AppIcon.appiconset/app_icon_*.png   macOS 应用图标，16–1024
//! app/windows/runner/resources/app_icon.ico                            同一个 Windows 图标，设置界面用
//! ```
//!
//! 小尺寸（32 像素及以下）用各自的 `-small.svg`：细节少、线条粗，缩小了不糊。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

type Result<T = ()> = std::result::Result<T, String>;

/// macOS 应用图标集里的尺寸。
const MACOS_SIZES: [u32; 7] = [16, 32, 64, 128, 256, 512, 1024];
/// Windows 图标文件里的尺寸。
const WINDOWS_SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];
/// 这个尺寸及以下用简化版。
const SMALL: u32 = 32;

pub(crate) fn run() -> Result {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("找不到仓库根目录")?
        .to_path_buf();
    let icons = root.join("assets/icons");

    for (svg, size, png) in [
        ("tray-connected.svg", 36, "tray-connected@2x.png"),
        ("tray-disconnected.svg", 36, "tray-disconnected@2x.png"),
        ("tray-win-connected.svg", 32, "tray-win-connected.png"),
        ("tray-win-disconnected.svg", 32, "tray-win-disconnected.png"),
    ] {
        render(&icons.join(svg), size, &icons.join(png))?;
    }

    let appiconset = root.join("app/macos/Runner/Assets.xcassets/AppIcon.appiconset");
    for size in MACOS_SIZES {
        let svg = if size <= SMALL {
            "app-icon-macos-small.svg"
        } else {
            "app-icon-macos.svg"
        };
        render(
            &icons.join(svg),
            size,
            &appiconset.join(format!("app_icon_{size}.png")),
        )?;
    }

    let scratch = root.join("target/icons");
    fs::create_dir_all(&scratch).map_err(|e| io(&scratch, e))?;
    let mut images = Vec::new();
    for size in WINDOWS_SIZES {
        let svg = if size <= SMALL {
            "app-icon-windows-small.svg"
        } else {
            "app-icon-windows.svg"
        };
        let png = scratch.join(format!("windows-{size}.png"));
        render(&icons.join(svg), size, &png)?;
        images.push((size, fs::read(&png).map_err(|e| io(&png, e))?));
    }
    let dmg = root.join("assets/dmg");
    render_size(
        &dmg.join("background.svg"),
        660,
        400,
        &dmg.join("background.png"),
    )?;
    let retina = dmg.join("background@2x.png");
    render_size(&dmg.join("background.svg"), 1320, 800, &retina)?;
    // Retina 那张标成 144 ppi，打包时 tiffutil 才把两张认成同一张图的两种分辨率。
    let marked = Command::new("sips")
        .args(["-s", "dpiWidth", "144", "-s", "dpiHeight", "144"])
        .arg(&retina)
        .output()
        .map_err(|e| format!("无法运行 sips：{e}"))?;
    if !marked.status.success() {
        return Err("sips 标注 background@2x.png 的分辨率失败".into());
    }

    let packed = pack_ico(&images)?;
    for ico in [
        icons.join("app-icon-windows.ico"),
        root.join("app/windows/runner/resources/app_icon.ico"),
    ] {
        fs::write(&ico, &packed).map_err(|e| io(&ico, e))?;
    }
    println!("✓ 图标已导出");
    Ok(())
}

/// 用 rsvg-convert 把 SVG 画成 `size`×`size` 的 PNG。
fn render(svg: &Path, size: u32, png: &Path) -> Result {
    render_size(svg, size, size, png)
}

fn render_size(svg: &Path, width: u32, height: u32, png: &Path) -> Result {
    let (w, h) = (width.to_string(), height.to_string());
    let status = Command::new("rsvg-convert")
        .args(["-w", &w, "-h", &h, "-o"])
        .arg(png)
        .arg(svg)
        .status()
        .map_err(|e| format!("无法运行 rsvg-convert（brew install librsvg）：{e}"))?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| format!("rsvg-convert 导出 {} 失败", svg.display()))
}

/// 把几张 PNG 打包成一个 ICO：文件头、每张图一条目录项，后面依次是 PNG 原文。
/// 从 Windows Vista 起，ICO 里的每张图都可以直接是 PNG。
fn pack_ico(images: &[(u32, Vec<u8>)]) -> Result<Vec<u8>> {
    let count = u16::try_from(images.len()).map_err(|_| "图太多")?;
    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes()); // 保留
    out.extend_from_slice(&1u16.to_le_bytes()); // 类型：图标
    out.extend_from_slice(&count.to_le_bytes());
    let mut offset = 6 + 16 * images.len();
    for (size, png) in images {
        // 宽高各占一个字节，256 记作 0。
        let side = if *size >= 256 { 0 } else { *size as u8 };
        out.extend_from_slice(&[side, side, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes()); // 平面数
        out.extend_from_slice(&32u16.to_le_bytes()); // 每像素位数
        let len = u32::try_from(png.len()).map_err(|_| "图太大")?;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&u32::try_from(offset).map_err(|_| "图太大")?.to_le_bytes());
        offset += png.len();
    }
    for (_, png) in images {
        out.extend_from_slice(png);
    }
    Ok(out)
}

fn io(path: &Path, e: std::io::Error) -> String {
    format!("{}：{e}", PathBuf::from(path).display())
}
