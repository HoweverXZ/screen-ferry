//! 浮层的绘制描述：画哪些形状和文字、哪些地方可以点。
//!
//! 样子在共用代码里算好，平台只照着画，所以两个系统上看起来一样。
//! 坐标以浮层窗口的左上角为原点，单位是“点”：平台按显示器的缩放比例换算成像素。

/// 颜色，分量 0–255，未预乘透明度。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// `0xRRGGBB` 加上透明度。
    pub const fn hex(rgb: u32, a: u8) -> Self {
        Self::new((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, a)
    }

    /// 把透明度乘以 `factor`（0–1）。
    pub fn fade(self, factor: f32) -> Self {
        let a = (f32::from(self.a) * factor.clamp(0.0, 1.0)).round() as u8;
        Self { a, ..self }
    }
}

/// 浮点矩形，左上角原点。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Frame {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Frame {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    /// 四边各向外扩 `dx`、`dy`（负数为向内收）。
    pub fn inflate(&self, dx: f32, dy: f32) -> Self {
        Self::new(
            self.x - dx,
            self.y - dy,
            self.w + 2.0 * dx,
            self.h + 2.0 * dy,
        )
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }
}

/// 字重。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Weight {
    Regular,
    Semibold,
}

/// 文字在框内的对齐方式。都只排一行，放不下时结尾显示省略号。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Align {
    Left,
    Center,
}

/// 一个要画的东西。
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// 填充的圆角矩形。圆角半径大于短边一半时按一半算，即胶囊形。
    RoundRect {
        frame: Frame,
        radius: f32,
        fill: Rgba,
    },
    /// 描边的圆角矩形。`dash` 为虚线的段长，0 为实线。
    RoundRectStroke {
        frame: Frame,
        radius: f32,
        width: f32,
        color: Rgba,
        dash: f32,
    },
    /// 填充的圆。
    Circle {
        cx: f32,
        cy: f32,
        r: f32,
        fill: Rgba,
    },
    /// 圆弧描边：从 12 点钟方向顺时针画 `sweep` 圈（0–1），圆头。
    Arc {
        cx: f32,
        cy: f32,
        r: f32,
        width: f32,
        sweep: f32,
        color: Rgba,
    },
    /// 折线描边，圆头圆角。
    Polyline {
        points: Vec<(f32, f32)>,
        width: f32,
        color: Rgba,
    },
    /// 一行文字，在 `frame` 内垂直居中。
    Text {
        frame: Frame,
        text: String,
        size: f32,
        weight: Weight,
        color: Rgba,
        align: Align,
    },
}

/// 一块可以点的区域。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub frame: Frame,
    pub id: u32,
}

/// 一帧浮层。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    /// 窗口大小（点）。
    pub width: f32,
    pub height: f32,
    /// 按顺序画，后画的盖住先画的。
    pub shapes: Vec<Shape>,
    pub hits: Vec<Hit>,
}
