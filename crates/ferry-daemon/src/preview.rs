//! 调试：把边缘投递胶囊的各种样子画成 PNG，用来核对样子，不需要真的拖文件。

use std::path::Path;

use ferry_core::Side;
use ferry_core::capsule::{Action, Avatar, Capsule, Glyph, Look, Tile, place};
use ferry_types::{Frame, Rgba, Scene, Shape};

type AppResult = Result<(), Box<dyn std::error::Error>>;

/// 设置界面默认的强调色。
const ACCENT: Rgba = Rgba::hex(0x0A6CFF, 255);
/// 预览用的桌面：与交互原型的 Mac 桌面同色。
const WALLPAPER: Rgba = Rgba::hex(0xD5DDEE, 255);

fn lines(a: &str, b: &str, c: &str) -> [String; 3] {
    [a.into(), b.into(), c.into()]
}

/// 预览的各种样子：文件名、贴在哪条边、样子。
fn samples() -> Vec<(&'static str, Side, Look)> {
    let doc = Tile::Doc {
        ext: "P12".into(),
        tone: Rgba::hex(0x0F766E, 255),
    };
    let wide = |avatar, text: [String; 3], hot, offline, action| Look::Wide {
        avatar,
        lines: text,
        hot,
        offline,
        action,
    };
    let tall = |glyph, sending, offline, closeness| Look::Tall {
        glyph,
        sending,
        offline,
        closeness,
    };
    vec![
        (
            "01-发送-靠近",
            Side::Left,
            tall(Glyph::Monitor, true, false, 0.3),
        ),
        (
            "02-发送-更近",
            Side::Left,
            tall(Glyph::Monitor, true, false, 1.0),
        ),
        (
            "03-发送-松手发送",
            Side::Left,
            wide(
                Avatar::Device(Glyph::Monitor),
                lines("松手发送到", "BF-202610101159", "证书.p12 · 3.2 KB"),
                true,
                false,
                Action::None,
            ),
        ),
        (
            "04-发送-未连接",
            Side::Left,
            wide(
                Avatar::Device(Glyph::Monitor),
                lines("未连接", "BF-202610101159", "连上后才能发送"),
                false,
                true,
                Action::None,
            ),
        ),
        (
            "05-发送-进度",
            Side::Left,
            wide(
                Avatar::Ring(0.46),
                lines("发送中 46%", "发布录屏.mov", "593 MB / 1.2 GB"),
                false,
                false,
                Action::Cancel,
            ),
        ),
        (
            "06-发送-完成",
            Side::Left,
            wide(
                Avatar::Check,
                lines("已发送", "到 BF-202610101159", ""),
                false,
                false,
                Action::None,
            ),
        ),
        (
            "07-传输中断",
            Side::Left,
            wide(
                Avatar::Warn,
                lines("传输中断", "发布录屏.mov", "重新连上后接着传"),
                false,
                false,
                Action::None,
            ),
        ),
        (
            "08-已取消",
            Side::Left,
            wide(
                Avatar::Cross,
                lines("已取消", "没有发送", ""),
                false,
                false,
                Action::None,
            ),
        ),
        (
            "09-接收-准备",
            Side::Right,
            tall(Glyph::Laptop, false, false, 0.6),
        ),
        (
            "10-接收-松手即接收",
            Side::Right,
            wide(
                Avatar::Device(Glyph::Laptop),
                lines("来自 MacBook", "松手即接收", "证书.p12 · 3.2 KB"),
                true,
                false,
                Action::None,
            ),
        ),
        (
            "11-接收-进度",
            Side::Right,
            wide(
                Avatar::Ring(0.72),
                lines("接收中 72%", "设计稿 等 3 项", "61 MB / 84 MB"),
                false,
                false,
                Action::Cancel,
            ),
        ),
        (
            "12-接收-卡片",
            Side::Right,
            Look::Card {
                from: "来自 MacBook".into(),
                name: "证书.p12".into(),
                detail: "已保存到「下载」· 3.2 KB".into(),
                tile: doc,
                chip: false,
            },
        ),
        (
            "13-接收-卡片-多项",
            Side::Right,
            Look::Card {
                from: "来自 BF-202610101159".into(),
                name: "设计稿 等 3 项".into(),
                detail: "已保存到「下载」· 86.4 MB".into(),
                tile: Tile::Stack(3),
                chip: true,
            },
        ),
        (
            "14-上边-靠近",
            Side::Top,
            tall(Glyph::Laptop, true, false, 0.5),
        ),
    ]
}

/// 每种样子动画停下后的一帧，铺上桌面颜色作底。
fn scenes() -> Result<Vec<(&'static str, Scene)>, Box<dyn std::error::Error>> {
    let screen = Frame::new(0.0, 0.0, 1440.0, 900.0);
    let mut out = Vec::new();
    for (name, side, look) in samples() {
        let mut capsule = Capsule::new();
        capsule.show(look.clone(), side, place(&look, side, screen, 450.0, 1.0));
        for _ in 0..240 {
            if !capsule.step(1.0 / 60.0) {
                break;
            }
        }
        let (_, mut scene) = capsule.render(1.0, ACCENT).ok_or("胶囊没有显示出来")?;
        let background = Shape::RoundRect {
            frame: Frame::new(0.0, 0.0, scene.width, scene.height),
            radius: 0.0,
            fill: WALLPAPER,
        };
        scene.shapes.insert(0, background);
        out.push((name, scene));
    }
    Ok(out)
}

/// 画出所有样子，每个一张 PNG（2 倍分辨率）。
#[cfg(target_os = "macos")]
pub(crate) fn write(dir: &Path) -> AppResult {
    std::fs::create_dir_all(dir)?;
    for (name, scene) in scenes()? {
        let png = ferry_platform_macos::render_png(&scene, 2.0).ok_or("画不出来")?;
        let path = dir.join(format!("{name}.png"));
        std::fs::write(&path, png)?;
        println!("{}", path.display());
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn write(_dir: &Path) -> AppResult {
    scenes()?;
    Err("胶囊预览目前只在 macOS 上可用".into())
}
