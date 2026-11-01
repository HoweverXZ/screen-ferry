//! 把胶囊的一帧画成形状和文字。尺寸、颜色与交互原型一致。

use ferry_types::{Align, Frame, Hit, Rgba, Scene, Shape, Weight};

use super::{Action, Avatar, Glyph, Look, Tile, hit};
use crate::layout::Side;

const BODY: Rgba = Rgba::new(28, 27, 31, 237);
const BODY_OFFLINE: Rgba = Rgba::new(98, 95, 103, 240);
const WHITE: Rgba = Rgba::new(255, 255, 255, 255);
const SOFT: Rgba = Rgba::new(255, 255, 255, 173);
const TINT: Rgba = Rgba::new(255, 255, 255, 31);
const SHADOW: Rgba = Rgba::new(24, 18, 44, 255);
const GOOD_BG: Rgba = Rgba::hex(0xA8EDA3, 255);
const GOOD_INK: Rgba = Rgba::hex(0x14561B, 255);
const WARN_BG: Rgba = Rgba::hex(0xFFD9A6, 255);
const WARN_INK: Rgba = Rgba::hex(0x7A4200, 255);

/// 一个画笔：把透明度统一乘上去，再记下形状和可点的区域。
struct Pen {
    opacity: f32,
    shapes: Vec<Shape>,
    hits: Vec<Hit>,
}

impl Pen {
    fn fill(&mut self, frame: Frame, radius: f32, color: Rgba) {
        let fill = color.fade(self.opacity);
        self.shapes.push(Shape::RoundRect {
            frame,
            radius,
            fill,
        });
    }

    fn circle(&mut self, cx: f32, cy: f32, r: f32, color: Rgba) {
        let fill = color.fade(self.opacity);
        self.shapes.push(Shape::Circle { cx, cy, r, fill });
    }

    fn line(&mut self, points: Vec<(f32, f32)>, width: f32, color: Rgba) {
        let color = color.fade(self.opacity);
        self.shapes.push(Shape::Polyline {
            points,
            width,
            color,
        });
    }

    fn text(
        &mut self,
        frame: Frame,
        text: &str,
        size: f32,
        weight: Weight,
        color: Rgba,
        align: Align,
    ) {
        if text.is_empty() {
            return;
        }
        self.shapes.push(Shape::Text {
            frame,
            text: text.to_owned(),
            size,
            weight,
            color: color.fade(self.opacity),
            align,
        });
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn paint(
    look: &Look,
    side: Side,
    width: f32,
    height: f32,
    body: Frame,
    content: f32,
    opacity: f32,
    accent: Rgba,
) -> Scene {
    let mut pen = Pen {
        opacity,
        shapes: Vec::new(),
        hits: Vec::new(),
    };
    let radius = match look {
        Look::Card { .. } => 26.0,
        _ => body.w.min(body.h) / 2.0,
    };

    // 柔和的阴影：几圈越来越淡的圆角矩形，向下偏一点。
    for (i, alpha) in [14u8, 11, 8, 6, 4, 2].into_iter().enumerate() {
        let grow = 2.0 * (i as f32 + 1.0);
        let frame = Frame::new(body.x, body.y + 8.0, body.w, body.h).inflate(grow, grow);
        pen.fill(frame, radius + grow, SHADOW.fade(f32::from(alpha) / 255.0));
    }
    let offline = matches!(
        look,
        Look::Tall { offline: true, .. } | Look::Wide { offline: true, .. }
    );
    pen.fill(body, radius, if offline { BODY_OFFLINE } else { BODY });
    if matches!(look, Look::Wide { hot: true, .. }) {
        let color = accent.fade(opacity);
        pen.shapes.push(Shape::RoundRectStroke {
            frame: body.inflate(1.0, 1.0),
            radius: radius + 1.0,
            width: 2.0,
            color,
            dash: 0.0,
        });
    }

    let mut inner = Pen {
        opacity: opacity * content,
        shapes: Vec::new(),
        hits: Vec::new(),
    };
    match look {
        Look::Tall {
            glyph,
            sending,
            offline,
            ..
        } => tall(&mut inner, body, *glyph, *sending, *offline, side),
        Look::Wide {
            avatar,
            lines,
            offline,
            action,
            ..
        } => wide(&mut inner, body, *avatar, lines, *offline, *action, accent),
        Look::Card {
            from,
            name,
            detail,
            tile,
            chip,
        } => card(&mut inner, body, from, name, detail, tile, *chip, accent),
    }
    pen.shapes.append(&mut inner.shapes);
    // 内容还没显示出来时不能点。
    if content > 0.5 && opacity > 0.5 {
        pen.hits.append(&mut inner.hits);
    }
    Scene {
        width,
        height,
        shapes: pen.shapes,
        hits: pen.hits,
    }
}

fn tall(pen: &mut Pen, body: Frame, glyph: Glyph, sending: bool, offline: bool, side: Side) {
    let horizontal = body.w > body.h;
    let (cx, cy) = (body.x + body.w / 2.0, body.y + body.h / 2.0);
    // 设备图标和下面的箭头（或圆点）排成一列；横放时排成一行。
    let (gx, gy, mx, my) = if horizontal {
        (cx - 14.0, cy, cx + 14.0, cy)
    } else {
        (cx, cy - 14.0, cx, cy + 14.0)
    };
    device(pen, gx, gy, 22.0, glyph, offline, WHITE);
    if sending {
        // 箭头指向对方所在的那条边。
        let (dx, dy) = match side {
            Side::Left => (-1.0, 0.0),
            Side::Right => (1.0, 0.0),
            Side::Top => (0.0, -1.0),
            Side::Bottom => (0.0, 1.0),
        };
        let (s, b) = (3.5, 7.0);
        let tip = (mx + dx * s, my + dy * s);
        let wing = |sign: f32| (mx - dx * s + dy * b * sign, my - dy * s + dx * b * sign);
        pen.line(vec![wing(1.0), tip, wing(-1.0)], 1.8, WHITE);
    } else {
        for i in -1..=1 {
            let o = i as f32 * 8.0;
            let (x, y) = if horizontal {
                (mx + o, my)
            } else {
                (mx, my + o)
            };
            pen.circle(x, y, 2.0, WHITE.fade(0.7 - 0.2 * i as f32));
        }
    }
}

fn wide(
    pen: &mut Pen,
    body: Frame,
    avatar: Avatar,
    lines: &[String; 3],
    offline: bool,
    action: Action,
    accent: Rgba,
) {
    let (ax, ay) = (body.x + 14.0 + 22.0, body.y + body.h / 2.0);
    match avatar {
        Avatar::Device(glyph) => {
            pen.circle(ax, ay, 22.0, TINT.fade(if offline { 0.8 } else { 1.0 }));
            device(pen, ax, ay, 22.0, glyph, offline, WHITE);
        }
        Avatar::Ring(progress) => {
            let color = TINT.fade(pen.opacity);
            pen.shapes.push(Shape::Arc {
                cx: ax,
                cy: ay,
                r: 20.0,
                width: 4.0,
                sweep: 1.0,
                color,
            });
            let color = accent.fade(pen.opacity);
            pen.shapes.push(Shape::Arc {
                cx: ax,
                cy: ay,
                r: 20.0,
                width: 4.0,
                sweep: progress.clamp(0.0, 1.0),
                color,
            });
            let pct = format!("{}%", (progress.clamp(0.0, 1.0) * 100.0).round() as u32);
            let frame = Frame::new(ax - 18.0, ay - 8.0, 36.0, 16.0);
            pen.text(frame, &pct, 10.5, Weight::Semibold, WHITE, Align::Center);
        }
        Avatar::Check => {
            pen.circle(ax, ay, 22.0, GOOD_BG);
            pen.line(
                icon(ax, ay, 22.0, &[(5.0, 12.5), (9.5, 17.0), (19.0, 7.5)]),
                2.0,
                GOOD_INK,
            );
        }
        Avatar::Warn => {
            pen.circle(ax, ay, 22.0, WARN_BG);
            pen.line(
                icon(ax, ay, 22.0, &[(12.0, 7.0), (12.0, 13.0)]),
                2.0,
                WARN_INK,
            );
            pen.circle(ax, ay + 4.8, 1.3, WARN_INK);
        }
        Avatar::Cross => {
            pen.circle(ax, ay, 22.0, TINT.fade(1.15));
            cross(pen, ax, ay, 20.0, WHITE);
        }
    }

    let right = match action {
        Action::None => body.right() - 14.0,
        Action::Cancel => {
            let (cx, cy) = (body.right() - 14.0 - 15.0, body.y + body.h / 2.0);
            pen.circle(cx, cy, 15.0, TINT.fade(1.15));
            cross(pen, cx, cy, 16.0, WHITE);
            pen.hits.push(Hit {
                frame: Frame::new(cx - 18.0, cy - 18.0, 36.0, 36.0),
                id: hit::CANCEL,
            });
            cx - 15.0 - 12.0
        }
    };
    let x = body.x + 14.0 + 44.0 + 12.0;
    let w = (right - x).max(0.0);
    let three = !lines[2].is_empty();
    let total = if three {
        15.0 + 19.0 + 15.0
    } else {
        15.0 + 19.0
    };
    let mut y = body.y + (body.h - total) / 2.0;
    pen.text(
        Frame::new(x, y, w, 15.0),
        &lines[0],
        11.5,
        Weight::Regular,
        SOFT,
        Align::Left,
    );
    y += 15.0;
    pen.text(
        Frame::new(x, y, w, 19.0),
        &lines[1],
        14.0,
        Weight::Semibold,
        WHITE,
        Align::Left,
    );
    y += 19.0;
    if three {
        pen.text(
            Frame::new(x, y, w, 15.0),
            &lines[2],
            11.5,
            Weight::Regular,
            SOFT,
            Align::Left,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn card(
    pen: &mut Pen,
    body: Frame,
    from: &str,
    name: &str,
    detail: &str,
    tile: &Tile,
    chip: bool,
    accent: Rgba,
) {
    let (x0, y0) = (body.x + 16.0, body.y + 16.0);
    draw_tile(pen, x0, y0, tile);

    let close = (body.right() - 16.0 - 13.0, y0 + 13.0);
    pen.circle(close.0, close.1, 13.0, TINT);
    cross(pen, close.0, close.1, 14.0, WHITE);
    pen.hits.push(Hit {
        frame: Frame::new(close.0 - 16.0, close.1 - 16.0, 32.0, 32.0),
        id: hit::CLOSE,
    });

    let tx = x0 + 44.0 + 12.0;
    let tw = (close.0 - 13.0 - 8.0 - tx).max(0.0);
    pen.text(
        Frame::new(tx, y0 - 1.0, tw, 15.0),
        from,
        11.5,
        Weight::Regular,
        SOFT,
        Align::Left,
    );
    pen.text(
        Frame::new(tx, y0 + 14.0, tw, 19.0),
        name,
        14.0,
        Weight::Semibold,
        WHITE,
        Align::Left,
    );
    pen.text(
        Frame::new(tx, y0 + 33.0, tw, 15.0),
        detail,
        11.5,
        Weight::Regular,
        SOFT,
        Align::Left,
    );

    let by = y0 + 44.0 + 12.0;
    let bw = (body.w - 32.0 - 8.0) / 2.0;
    let open = Frame::new(x0, by, bw, 32.0);
    let reveal = Frame::new(x0 + bw + 8.0, by, bw, 32.0);
    pen.fill(open, 16.0, accent);
    pen.text(open, "打开", 12.5, Weight::Semibold, WHITE, Align::Center);
    pen.fill(reveal, 16.0, TINT.fade(1.15));
    pen.text(
        reveal,
        "在文件夹中显示",
        12.5,
        Weight::Semibold,
        WHITE,
        Align::Center,
    );
    pen.hits.push(Hit {
        frame: open,
        id: hit::OPEN,
    });
    pen.hits.push(Hit {
        frame: reveal,
        id: hit::REVEAL,
    });

    if chip {
        let frame = Frame::new(x0, by + 32.0 + 12.0, body.w - 32.0, 34.0);
        let color = WHITE.fade(0.32 * pen.opacity);
        pen.shapes.push(Shape::RoundRectStroke {
            frame,
            radius: 12.0,
            width: 1.5,
            color,
            dash: 4.0,
        });
        pen.text(
            frame,
            "按住这里，拖到任意文件夹",
            12.0,
            Weight::Regular,
            WHITE.fade(0.86),
            Align::Center,
        );
        pen.hits.push(Hit {
            frame,
            id: hit::CHIP,
        });
    }
}

fn draw_tile(pen: &mut Pen, x: f32, y: f32, tile: &Tile) {
    match tile {
        Tile::Folder => {
            pen.fill(
                Frame::new(x + 2.0, y + 6.0, 17.0, 9.0),
                4.0,
                Rgba::hex(0x5E93E0, 255),
            );
            pen.fill(
                Frame::new(x + 2.0, y + 11.0, 40.0, 29.0),
                6.0,
                Rgba::hex(0x78A9EE, 255),
            );
        }
        Tile::Doc { ext, tone } => page(pen, x + 5.0, y + 1.0, ext, *tone),
        Tile::Stack(n) => {
            pen.fill(Frame::new(x + 11.0, y, 30.0, 38.0), 6.0, WHITE.fade(0.45));
            pen.fill(
                Frame::new(x + 7.0, y + 3.0, 30.0, 38.0),
                6.0,
                WHITE.fade(0.7),
            );
            page(
                pen,
                x + 3.0,
                y + 2.0,
                &n.to_string(),
                Rgba::hex(0x6A4BD8, 255),
            );
        }
    }
}

/// 一页纸：白底，底部是带扩展名的色条。
fn page(pen: &mut Pen, x: f32, y: f32, ext: &str, tone: Rgba) {
    pen.fill(Frame::new(x, y, 34.0, 42.0), 6.0, WHITE);
    pen.fill(Frame::new(x, y + 28.0, 34.0, 14.0), 6.0, tone);
    pen.fill(Frame::new(x, y + 28.0, 34.0, 6.0), 0.0, tone);
    pen.text(
        Frame::new(x, y + 28.0, 34.0, 14.0),
        ext,
        8.0,
        Weight::Semibold,
        WHITE,
        Align::Center,
    );
}

/// 设备图标：以 (cx, cy) 为中心、`size` 大小，按 24 格的设计稿缩放。
fn device(pen: &mut Pen, cx: f32, cy: f32, size: f32, glyph: Glyph, offline: bool, color: Rgba) {
    let k = size / 24.0;
    let width = 1.8 * k;
    let at = |x: f32, y: f32| (cx + (x - 12.0) * k, cy + (y - 12.0) * k);
    match glyph {
        Glyph::Monitor => {
            rect_outline(pen, at(3.0, 4.0), 18.0 * k, 12.0 * k, 2.0 * k, width, color);
            pen.line(vec![at(9.0, 20.0), at(15.0, 20.0)], width, color);
            pen.line(vec![at(12.0, 16.0), at(12.0, 20.0)], width, color);
        }
        Glyph::Laptop => {
            rect_outline(pen, at(4.0, 5.0), 16.0 * k, 11.0 * k, 2.0 * k, width, color);
            pen.line(vec![at(2.0, 19.0), at(22.0, 19.0)], width, color);
        }
    }
    if offline {
        pen.line(vec![at(4.0, 2.0), at(20.0, 22.0)], width, color);
    }
}

fn rect_outline(
    pen: &mut Pen,
    (x, y): (f32, f32),
    w: f32,
    h: f32,
    radius: f32,
    width: f32,
    color: Rgba,
) {
    let color = color.fade(pen.opacity);
    pen.shapes.push(Shape::RoundRectStroke {
        frame: Frame::new(x, y, w, h),
        radius,
        width,
        color,
        dash: 0.0,
    });
}

/// 叉号，大小 `size`，以 (cx, cy) 为中心。
fn cross(pen: &mut Pen, cx: f32, cy: f32, size: f32, color: Rgba) {
    let d = size * 5.0 / 24.0;
    pen.line(vec![(cx - d, cy - d), (cx + d, cy + d)], 1.8, color);
    pen.line(vec![(cx + d, cy - d), (cx - d, cy + d)], 1.8, color);
}

/// 把 24 格设计稿上的点换算到以 (cx, cy) 为中心、`size` 大小的位置。
fn icon(cx: f32, cy: f32, size: f32, points: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let k = size / 24.0;
    points
        .iter()
        .map(|&(x, y)| (cx + (x - 12.0) * k, cy + (y - 12.0) * k))
        .collect()
}
