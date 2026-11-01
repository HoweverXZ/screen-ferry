//! 边缘投递胶囊：每种样子的大小、贴着哪条边放在哪里、怎么动画过去，以及每一帧画什么。
//!
//! 引擎决定显示哪种 [`Look`]、贴在哪里；这里算出动画中的每一帧，交给平台照着画。
//! 尺寸以“点”为单位设计，`scale` 把它换算成桌面坐标（macOS 为 1，Windows 为系统缩放比例）。

mod paint;
mod spring;

use ferry_types::{Frame, Rgba, Scene};

use crate::layout::Side;
use spring::Spring;

/// 胶囊离屏幕边缘的距离（点）。
const MARGIN: f32 = 10.0;
/// 窗口比胶囊大出的一圈，用来画阴影（点）。
pub const SHADOW: f32 = 20.0;
/// 胶囊离屏幕上下（或左右）两端至少留出的距离（点），避开菜单栏和程序坞。
const END_GAP: f32 = 34.0;

/// 可以点的地方。
pub mod hit {
    pub const CANCEL: u32 = 1;
    pub const OPEN: u32 = 2;
    pub const REVEAL: u32 = 3;
    pub const CLOSE: u32 = 4;
    /// 卡片上可以把文件拖出去的那一条。
    pub const CHIP: u32 = 5;
}

/// 胶囊的样子。
#[derive(Clone, Debug, PartialEq)]
pub enum Look {
    /// 拖着文件靠近边缘：贴边的细长胶囊，越靠近越大（`closeness` 0–1）。
    /// 发送端显示指向对方的箭头，接收端显示等待的圆点。
    Tall {
        glyph: Glyph,
        sending: bool,
        offline: bool,
        closeness: f32,
    },
    /// 一行横向的胶囊：左边一个图标，右边两到三行字，最右边可能有一个按钮。
    Wide {
        avatar: Avatar,
        /// 小字、大字、小字；第三行为空时只显示两行。
        lines: [String; 3],
        /// 描一圈强调色：可以松手了。
        hot: bool,
        offline: bool,
        action: Action,
    },
    /// 收到文件后的卡片。
    Card {
        from: String,
        name: String,
        detail: String,
        tile: Tile,
        /// 是否显示“拖到任意文件夹”那一条。
        chip: bool,
    },
}

/// 设备图标。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    Monitor,
    Laptop,
}

/// 横向胶囊左边的圆形图标。
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Avatar {
    Device(Glyph),
    /// 进度环，0–1。
    Ring(f32),
    Check,
    Warn,
    Cross,
}

/// 横向胶囊右边的按钮。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    None,
    Cancel,
}

/// 卡片上代表收到内容的小图标。
#[derive(Clone, Debug, PartialEq)]
pub enum Tile {
    /// 文件：带扩展名色条的一页纸。
    Doc {
        ext: String,
        tone: Rgba,
    },
    Folder,
    /// 多项内容。
    Stack(u32),
}

impl Look {
    /// 胶囊本身的大小（点），贴在左右边缘时。
    pub fn size(&self) -> (f32, f32) {
        match self {
            Self::Tall { closeness, .. } => {
                let k = closeness.clamp(0.0, 1.0);
                (34.0 + 24.0 * k, 92.0 + 76.0 * k)
            }
            Self::Wide {
                avatar,
                lines,
                action,
                ..
            } => {
                if *action != Action::None || matches!(avatar, Avatar::Ring(_)) {
                    (304.0, 82.0)
                } else if lines[2].is_empty() {
                    (236.0, 64.0)
                } else {
                    (284.0, 82.0)
                }
            }
            Self::Card { chip, .. } => (330.0, if *chip { 164.0 } else { 120.0 }),
        }
    }
}

/// 胶囊贴在屏幕 `screen`（桌面坐标）朝向对方的 `side` 边上时的位置。
///
/// `along` 是沿着这条边的坐标（左右边为 y，上下边为 x），胶囊尽量以它为中心。
/// 上下边上的细长胶囊横过来放。
pub fn place(look: &Look, side: Side, screen: Frame, along: f32, scale: f32) -> Frame {
    let (mut w, mut h) = look.size();
    if matches!(look, Look::Tall { .. }) && matches!(side, Side::Top | Side::Bottom) {
        (w, h) = (h, w);
    }
    let (w, h, margin, gap) = (w * scale, h * scale, MARGIN * scale, END_GAP * scale);
    let along_y = || {
        clamp(
            along - h / 2.0,
            screen.y + gap,
            screen.bottom() - h - margin,
        )
    };
    let along_x = || {
        clamp(
            along - w / 2.0,
            screen.x + margin,
            screen.right() - w - margin,
        )
    };
    match side {
        Side::Left => Frame::new(screen.x + margin, along_y(), w, h),
        Side::Right => Frame::new(screen.right() - margin - w, along_y(), w, h),
        Side::Top => Frame::new(along_x(), screen.y + gap, w, h),
        Side::Bottom => Frame::new(along_x(), screen.bottom() - margin - h, w, h),
    }
}

/// 动画中的胶囊。
#[derive(Clone, Debug)]
pub struct Capsule {
    /// 正在显示（或正在淡出时最后显示）的样子。
    look: Option<Look>,
    /// 指向对方的那条边，决定箭头方向。
    side: Side,
    x: Spring,
    y: Spring,
    w: Spring,
    h: Spring,
    opacity: Spring,
}

impl Default for Capsule {
    fn default() -> Self {
        Self::new()
    }
}

impl Capsule {
    pub fn new() -> Self {
        Self {
            look: None,
            side: Side::Left,
            x: Spring::bouncy(0.0),
            y: Spring::bouncy(0.0),
            w: Spring::bouncy(0.0),
            h: Spring::bouncy(0.0),
            opacity: Spring::smooth(0.0),
        }
    }

    /// 显示成 `look`，动画到 `target`（桌面坐标）。原本没显示时从边缘上的一个小点长出来。
    pub fn show(&mut self, look: Look, side: Side, target: Frame) {
        if !self.is_visible() {
            let (cx, cy) = (target.x + target.w / 2.0, target.y + target.h / 2.0);
            let seed = target.w.min(target.h) * 0.3;
            let (sx, sy) = match side {
                Side::Left => (target.x, cy - seed / 2.0),
                Side::Right => (target.right() - seed, cy - seed / 2.0),
                Side::Top => (cx - seed / 2.0, target.y),
                Side::Bottom => (cx - seed / 2.0, target.bottom() - seed),
            };
            self.x.snap(sx);
            self.y.snap(sy);
            self.w.snap(seed);
            self.h.snap(seed);
        }
        self.side = side;
        self.look = Some(look);
        self.x.target = target.x;
        self.y.target = target.y;
        self.w.target = target.w;
        self.h.target = target.h;
        self.opacity.target = 1.0;
    }

    /// 淡出并收起。
    pub fn hide(&mut self) {
        self.opacity.target = 0.0;
        let seed = self.w.target.min(self.h.target) * 0.3;
        match self.side {
            Side::Left => {}
            Side::Right => self.x.target = self.x.target + self.w.target - seed,
            Side::Top => {}
            Side::Bottom => self.y.target = self.y.target + self.h.target - seed,
        }
        self.w.target = seed;
        self.h.target = seed;
    }

    /// 当前是否在屏幕上（包括正在淡出）。
    pub fn is_visible(&self) -> bool {
        self.look.is_some() && (self.opacity.target > 0.0 || self.opacity.value > 0.01)
    }

    /// 前进 `dt` 秒，返回是否还在动。淡出结束后忘掉样子。
    pub fn step(&mut self, dt: f32) -> bool {
        for spring in [
            &mut self.x,
            &mut self.y,
            &mut self.w,
            &mut self.h,
            &mut self.opacity,
        ] {
            spring.step(dt);
        }
        if self.opacity.target == 0.0 && self.opacity.value <= 0.01 {
            self.look = None;
            return false;
        }
        let still = self.x.settled(0.3)
            && self.y.settled(0.3)
            && self.w.settled(0.3)
            && self.h.settled(0.3)
            && self.opacity.settled(0.005);
        !still
    }

    /// 胶囊当前的位置和大小（桌面坐标）。
    pub fn frame(&self) -> Frame {
        Frame::new(
            self.x.value,
            self.y.value,
            self.w.value.max(1.0),
            self.h.value.max(1.0),
        )
    }

    /// 这一帧：窗口放在哪里（桌面坐标），里面画什么。没显示时为 `None`。
    pub fn render(&self, scale: f32, accent: Rgba) -> Option<(Frame, Scene)> {
        let look = self.look.as_ref()?;
        let body = self.frame();
        let window = body.inflate(SHADOW * scale, SHADOW * scale);
        // 大小离目标还远时（正在变形）先不画内容，免得文字挤成一团。
        let off = |v: &Spring| (v.value - v.target).abs() / v.target.max(1.0);
        let content = (1.0 - (off(&self.w) + off(&self.h)) * 4.0).clamp(0.0, 1.0);
        let local = Frame::new(SHADOW, SHADOW, body.w / scale, body.h / scale);
        let scene = paint::paint(
            look,
            self.side,
            window.w / scale,
            window.h / scale,
            local,
            content,
            self.opacity.value.clamp(0.0, 1.0),
            accent,
        );
        Some((window, scene))
    }
}

fn clamp(v: f32, lo: f32, hi: f32) -> f32 {
    if hi < lo { lo } else { v.clamp(lo, hi) }
}
