//! 用 Direct2D 和 DirectWrite 把 [`Scene`] 画进一张 32 位位图，再贴到分层窗口上。
//!
//! 位图与窗口一样大；场景以“点”为单位，乘上缩放比例变成像素。文字用微软雅黑，灰度抗锯齿
//! （透明窗口上不能用 ClearType），放不下时结尾显示省略号。

use std::collections::HashMap;

use ferry_platform::Result;
use ferry_types::{Align, Frame, Rgba, Scene, Shape, Weight};
use windows::Win32::Foundation::{COLORREF, HWND, POINT, RECT, SIZE};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D_SIZE_F, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_HOLLOW,
    D2D1_FIGURE_END_OPEN, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ARC_SEGMENT, D2D1_ARC_SIZE_LARGE, D2D1_ARC_SIZE_SMALL, D2D1_CAP_STYLE_FLAT,
    D2D1_CAP_STYLE_ROUND, D2D1_DASH_STYLE_CUSTOM, D2D1_DASH_STYLE_SOLID,
    D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_ELLIPSE, D2D1_FACTORY_TYPE_SINGLE_THREADED,
    D2D1_FEATURE_LEVEL_DEFAULT, D2D1_LINE_JOIN_ROUND, D2D1_RENDER_TARGET_PROPERTIES,
    D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE, D2D1_ROUNDED_RECT,
    D2D1_STROKE_STYLE_PROPERTIES, D2D1_SWEEP_DIRECTION_CLOCKWISE,
    D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE, D2D1CreateFactory, ID2D1DCRenderTarget, ID2D1Factory,
    ID2D1StrokeStyle,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_MEASURING_MODE_NATURAL,
    DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING,
    DWRITE_TRIMMING, DWRITE_TRIMMING_GRANULARITY_CHARACTER, DWRITE_WORD_WRAPPING_NO_WRAP,
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, HBITMAP, HDC,
    HGDIOBJ, SelectObject,
};
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SetWindowPos, ULW_ALPHA,
    UpdateLayeredWindow,
};
use windows::core::w;
use windows_numerics::{Matrix3x2, Vector2};

use super::os;

/// 整个窗口铺的底色：几乎透明、看不出来，但能接住鼠标和拖放。
const VEIL: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 1.0 / 255.0,
};

/// 与窗口一样大的位图，大小不变时重复使用。
struct Surface {
    dc: HDC,
    bitmap: HBITMAP,
    old: HGDIOBJ,
    width: i32,
    height: i32,
}

impl Drop for Surface {
    fn drop(&mut self) {
        // SAFETY: 这些 GDI 对象由本结构创建，只在界面线程上使用和释放。
        unsafe {
            SelectObject(self.dc, self.old);
            let _ = DeleteObject(self.bitmap.into());
            let _ = DeleteDC(self.dc);
        }
    }
}

pub(super) struct Painter {
    factory: ID2D1Factory,
    target: ID2D1DCRenderTarget,
    text: IDWriteFactory,
    round: ID2D1StrokeStyle,
    /// 按字号（十分之一点）、是否加粗、是否居中缓存的文字格式。
    formats: HashMap<(u32, bool, bool), IDWriteTextFormat>,
    surface: Option<Surface>,
}

impl Painter {
    pub(super) fn new() -> Result<Self> {
        // SAFETY: 创建 Direct2D 与 DirectWrite 的工厂和渲染目标，参数都是有效的常量。
        unsafe {
            let factory: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)
                .map_err(|e| os("D2D1CreateFactory", e))?;
            let props = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
            };
            let target = factory
                .CreateDCRenderTarget(&props)
                .map_err(|e| os("CreateDCRenderTarget", e))?;
            target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            let text: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)
                .map_err(|e| os("DWriteCreateFactory", e))?;
            let round = stroke_style(&factory, D2D1_CAP_STYLE_ROUND, None)?;
            Ok(Self {
                factory,
                target,
                text,
                round,
                formats: HashMap::new(),
                surface: None,
            })
        }
    }

    /// 把窗口放到 `frame`（桌面像素），画出 `scene`（每点 `scale` 个像素）；
    /// 没有 `scene` 时只铺一层看不见的底色，作为接收条。
    pub(super) fn present(
        &mut self,
        hwnd: HWND,
        frame: Frame,
        scale: f32,
        scene: Option<&Scene>,
    ) -> Result<()> {
        let width = frame.w.round().max(1.0) as i32;
        let height = frame.h.round().max(1.0) as i32;
        let dc = self.surface(width, height)?;
        // SAFETY: 位图与 DC 都由本结构持有；绘制都在 BeginDraw/EndDraw 之间。
        unsafe {
            let rect = RECT {
                left: 0,
                top: 0,
                right: width,
                bottom: height,
            };
            self.target.BindDC(dc, &rect).map_err(|e| os("BindDC", e))?;
            self.target.BeginDraw();
            self.target.SetTransform(&Matrix3x2::identity());
            self.target.Clear(Some(&D2D1_COLOR_F::default()));
            let veil = self
                .target
                .CreateSolidColorBrush(&VEIL, None)
                .map_err(|e| os("CreateSolidColorBrush", e))?;
            let all = D2D_RECT_F {
                left: 0.0,
                top: 0.0,
                right: width as f32,
                bottom: height as f32,
            };
            self.target.FillRectangle(&all, &veil);
            if let Some(scene) = scene {
                self.target.SetTransform(&Matrix3x2::scale(scale, scale));
                for shape in &scene.shapes {
                    // 某个形状画不出来时跳过它，不影响其余部分。
                    let _ = self.shape(shape);
                }
            }
            self.target
                .EndDraw(None, None)
                .map_err(|e| os("EndDraw", e))?;

            let at = POINT {
                x: frame.x.round() as i32,
                y: frame.y.round() as i32,
            };
            let size = SIZE {
                cx: width,
                cy: height,
            };
            let origin = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            UpdateLayeredWindow(
                hwnd,
                None,
                Some(&at),
                Some(&size),
                Some(dc),
                Some(&origin),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            )
            .map_err(|e| os("UpdateLayeredWindow", e))?;
            // 显示出来并保持在最上层，但不抢焦点。
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
        Ok(())
    }

    /// 取一张 `width`×`height` 的位图所在的 DC，大小变了才重新创建。
    fn surface(&mut self, width: i32, height: i32) -> Result<HDC> {
        if let Some(s) = &self.surface
            && s.width == width
            && s.height == height
        {
            return Ok(s.dc);
        }
        self.surface = None;
        // SAFETY: 创建一张自上而下的 32 位位图，选进新建的内存 DC。
        unsafe {
            let dc = CreateCompatibleDC(None);
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits = std::ptr::null_mut();
            let bitmap = match CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
            {
                Ok(bitmap) => bitmap,
                Err(e) => {
                    let _ = DeleteDC(dc);
                    return Err(os("CreateDIBSection", e));
                }
            };
            let old = SelectObject(dc, bitmap.into());
            self.surface = Some(Surface {
                dc,
                bitmap,
                old,
                width,
                height,
            });
            Ok(dc)
        }
    }

    /// 画一个形状。坐标已经由变换矩阵换算成像素。
    ///
    /// # Safety
    ///
    /// 只能在 BeginDraw 与 EndDraw 之间调用。
    unsafe fn shape(&mut self, shape: &Shape) -> Result<()> {
        // SAFETY: 调用方保证处于绘制过程中；所有指针都指向本函数里的局部值。
        unsafe {
            match shape {
                Shape::RoundRect {
                    frame,
                    radius,
                    fill,
                } => {
                    let brush = self.brush(*fill)?;
                    self.target
                        .FillRoundedRectangle(&rounded(*frame, *radius), &brush);
                }
                Shape::RoundRectStroke {
                    frame,
                    radius,
                    width,
                    color,
                    dash,
                } => {
                    let brush = self.brush(*color)?;
                    let style = if *dash > 0.0 {
                        // 虚线的段长以线宽为单位。
                        let unit = dash / width.max(0.1);
                        stroke_style(&self.factory, D2D1_CAP_STYLE_FLAT, Some(&[unit, unit]))?
                    } else {
                        self.round.clone()
                    };
                    self.target.DrawRoundedRectangle(
                        &rounded(*frame, *radius),
                        &brush,
                        *width,
                        &style,
                    );
                }
                Shape::Circle { cx, cy, r, fill } => {
                    let brush = self.brush(*fill)?;
                    self.target.FillEllipse(&ellipse(*cx, *cy, *r), &brush);
                }
                Shape::Arc {
                    cx,
                    cy,
                    r,
                    width,
                    sweep,
                    color,
                } => {
                    if *sweep <= 0.0 {
                        return Ok(());
                    }
                    let brush = self.brush(*color)?;
                    if *sweep >= 0.999 {
                        self.target.DrawEllipse(
                            &ellipse(*cx, *cy, *r),
                            &brush,
                            *width,
                            &self.round,
                        );
                        return Ok(());
                    }
                    // 从 12 点钟方向顺时针画。
                    let angle = sweep * std::f32::consts::TAU;
                    let start = Vector2 { X: *cx, Y: cy - r };
                    let end = Vector2 {
                        X: cx + r * angle.sin(),
                        Y: cy - r * angle.cos(),
                    };
                    let path = self
                        .factory
                        .CreatePathGeometry()
                        .map_err(|e| os("CreatePathGeometry", e))?;
                    let sink = path.Open().map_err(|e| os("ID2D1PathGeometry::Open", e))?;
                    sink.BeginFigure(start, D2D1_FIGURE_BEGIN_HOLLOW);
                    sink.AddArc(&D2D1_ARC_SEGMENT {
                        point: end,
                        size: D2D_SIZE_F {
                            width: *r,
                            height: *r,
                        },
                        rotationAngle: 0.0,
                        sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                        arcSize: if *sweep > 0.5 {
                            D2D1_ARC_SIZE_LARGE
                        } else {
                            D2D1_ARC_SIZE_SMALL
                        },
                    });
                    sink.EndFigure(D2D1_FIGURE_END_OPEN);
                    sink.Close()
                        .map_err(|e| os("ID2D1GeometrySink::Close", e))?;
                    self.target.DrawGeometry(&path, &brush, *width, &self.round);
                }
                Shape::Polyline {
                    points,
                    width,
                    color,
                } => {
                    let Some((first, rest)) = points.split_first() else {
                        return Ok(());
                    };
                    let brush = self.brush(*color)?;
                    let path = self
                        .factory
                        .CreatePathGeometry()
                        .map_err(|e| os("CreatePathGeometry", e))?;
                    let sink = path.Open().map_err(|e| os("ID2D1PathGeometry::Open", e))?;
                    sink.BeginFigure(
                        Vector2 {
                            X: first.0,
                            Y: first.1,
                        },
                        D2D1_FIGURE_BEGIN_HOLLOW,
                    );
                    for &(x, y) in rest {
                        sink.AddLine(Vector2 { X: x, Y: y });
                    }
                    sink.EndFigure(D2D1_FIGURE_END_OPEN);
                    sink.Close()
                        .map_err(|e| os("ID2D1GeometrySink::Close", e))?;
                    self.target.DrawGeometry(&path, &brush, *width, &self.round);
                }
                Shape::Text {
                    frame,
                    text,
                    size,
                    weight,
                    color,
                    align,
                } => {
                    let format = self.format(*size, *weight, *align)?;
                    let brush = self.brush(*color)?;
                    let utf16: Vec<u16> = text.encode_utf16().collect();
                    self.target.DrawText(
                        &utf16,
                        &format,
                        &rect(*frame),
                        &brush,
                        D2D1_DRAW_TEXT_OPTIONS_CLIP,
                        DWRITE_MEASURING_MODE_NATURAL,
                    );
                }
            }
        }
        Ok(())
    }

    fn brush(
        &self,
        color: Rgba,
    ) -> Result<windows::Win32::Graphics::Direct2D::ID2D1SolidColorBrush> {
        let k = |v: u8| f32::from(v) / 255.0;
        let color = D2D1_COLOR_F {
            r: k(color.r),
            g: k(color.g),
            b: k(color.b),
            a: k(color.a),
        };
        // SAFETY: 颜色是本函数里的局部值。
        unsafe { self.target.CreateSolidColorBrush(&color, None) }
            .map_err(|e| os("CreateSolidColorBrush", e))
    }

    fn format(&mut self, size: f32, weight: Weight, align: Align) -> Result<IDWriteTextFormat> {
        let semibold = weight == Weight::Semibold;
        let center = align == Align::Center;
        let key = ((size * 10.0).round() as u32, semibold, center);
        if let Some(format) = self.formats.get(&key) {
            return Ok(format.clone());
        }
        // SAFETY: 字体名和区域名是常量字符串。
        let format = unsafe {
            let format = self
                .text
                .CreateTextFormat(
                    w!("Microsoft YaHei UI"),
                    None,
                    if semibold {
                        DWRITE_FONT_WEIGHT_SEMI_BOLD
                    } else {
                        DWRITE_FONT_WEIGHT_NORMAL
                    },
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    size,
                    w!("zh-cn"),
                )
                .map_err(|e| os("CreateTextFormat", e))?;
            let _ = format.SetTextAlignment(if center {
                DWRITE_TEXT_ALIGNMENT_CENTER
            } else {
                DWRITE_TEXT_ALIGNMENT_LEADING
            });
            let _ = format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
            let _ = format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP);
            if let Ok(sign) = self.text.CreateEllipsisTrimmingSign(&format) {
                let trimming = DWRITE_TRIMMING {
                    granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
                    delimiter: 0,
                    delimiterCount: 0,
                };
                let _ = format.SetTrimming(&trimming, &sign);
            }
            format
        };
        self.formats.insert(key, format.clone());
        Ok(format)
    }
}

fn stroke_style(
    factory: &ID2D1Factory,
    cap: windows::Win32::Graphics::Direct2D::D2D1_CAP_STYLE,
    dashes: Option<&[f32]>,
) -> Result<ID2D1StrokeStyle> {
    let props = D2D1_STROKE_STYLE_PROPERTIES {
        startCap: cap,
        endCap: cap,
        dashCap: cap,
        lineJoin: D2D1_LINE_JOIN_ROUND,
        miterLimit: 10.0,
        dashStyle: if dashes.is_some() {
            D2D1_DASH_STYLE_CUSTOM
        } else {
            D2D1_DASH_STYLE_SOLID
        },
        dashOffset: 0.0,
    };
    // SAFETY: 属性和虚线数组都是调用期间有效的局部值。
    unsafe { factory.CreateStrokeStyle(&props, dashes) }.map_err(|e| os("CreateStrokeStyle", e))
}

fn rect(frame: Frame) -> D2D_RECT_F {
    D2D_RECT_F {
        left: frame.x,
        top: frame.y,
        right: frame.right(),
        bottom: frame.bottom(),
    }
}

fn rounded(frame: Frame, radius: f32) -> D2D1_ROUNDED_RECT {
    let r = radius.min(frame.w / 2.0).min(frame.h / 2.0).max(0.0);
    D2D1_ROUNDED_RECT {
        rect: rect(frame),
        radiusX: r,
        radiusY: r,
    }
}

fn ellipse(cx: f32, cy: f32, r: f32) -> D2D1_ELLIPSE {
    D2D1_ELLIPSE {
        point: Vector2 { X: cx, Y: cy },
        radiusX: r,
        radiusY: r,
    }
}
