//! 剪贴板格式转换：Windows 剪贴板的位图（`CF_DIB`）与 PNG 互转，Windows 的 HTML 剪贴板格式，
//! 读 PNG 图片的尺寸。
//!
//! 协议里的图片一律是 PNG，HTML 一律是 UTF-8。各平台读写剪贴板时用这里的函数换成系统自己的格式。

use std::io::Cursor;

/// PNG 文件开头的 8 个字节。
const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
/// 最多处理这么多像素（例如 16384 × 16384），再大的图片不转换。
const MAX_PIXELS: u64 = 1 << 28;
/// `BITMAPINFOHEADER` 的大小。
const INFO_HEADER: usize = 40;
/// `BI_RGB`、`BI_BITFIELDS`、`BI_ALPHABITFIELDS`。
const BI_RGB: u32 = 0;
const BI_BITFIELDS: u32 = 3;
const BI_ALPHABITFIELDS: u32 = 6;
/// 96 DPI 换算成每米的像素数，写进位图头。
const PIXELS_PER_METER: i32 = 3780;

/// 一张解码后的图片：逐行从上到下，每个像素 RGBA 各一个字节。
struct Rgba {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

/// PNG 图片的宽和高。只读文件头，不解码；不是 PNG 时返回 `None`。
pub fn png_size(png: &[u8]) -> Option<(u32, u32)> {
    if png.len() < 24 || png[..8] != PNG_SIGNATURE || &png[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(png[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(png[20..24].try_into().ok()?);
    (width > 0 && height > 0).then_some((width, height))
}

/// Windows 剪贴板的 `CF_DIB`（位图信息头加像素）→ PNG。
///
/// 支持 24 位和 32 位、自下而上和自上而下、`BI_RGB` 和 `BI_BITFIELDS`。
/// 32 位 `BI_RGB` 的第四个字节大多数程序不填（全是 0），这时当作不透明。
/// 认不出的位图返回 `None`。
pub fn dib_to_png(dib: &[u8]) -> Option<Vec<u8>> {
    let image = decode_dib(dib)?;
    encode_png(&image)
}

/// PNG → Windows 剪贴板的 `CF_DIB`：32 位、自下而上、`BI_RGB`。
///
/// 读 `CF_DIB` 的程序大多不看透明度，所以半透明的像素先叠在白底上。认不出的 PNG 返回 `None`。
pub fn png_to_dib(png: &[u8]) -> Option<Vec<u8>> {
    let image = decode_png(png)?;
    let (width, height) = (image.width as usize, image.height as usize);
    let row = width * 4;
    let mut dib = Vec::with_capacity(INFO_HEADER + row * height);
    let push_u32 = |dib: &mut Vec<u8>, v: u32| dib.extend_from_slice(&v.to_le_bytes());
    let push_i32 = |dib: &mut Vec<u8>, v: i32| dib.extend_from_slice(&v.to_le_bytes());
    push_u32(&mut dib, INFO_HEADER as u32);
    push_i32(&mut dib, i32::try_from(image.width).ok()?);
    // 高度为正：自下而上，第一行是图片的最下面一行。
    push_i32(&mut dib, i32::try_from(image.height).ok()?);
    dib.extend_from_slice(&1u16.to_le_bytes()); // 平面数
    dib.extend_from_slice(&32u16.to_le_bytes()); // 每像素位数
    push_u32(&mut dib, BI_RGB);
    push_u32(&mut dib, u32::try_from(row * height).ok()?);
    push_i32(&mut dib, PIXELS_PER_METER);
    push_i32(&mut dib, PIXELS_PER_METER);
    push_u32(&mut dib, 0); // 调色板颜色数
    push_u32(&mut dib, 0); // 重要颜色数
    for y in (0..height).rev() {
        for px in image.pixels[y * row..(y + 1) * row].chunks_exact(4) {
            let a = u32::from(px[3]);
            let over_white = |c: u8| ((u32::from(c) * a + 255 * (255 - a) + 127) / 255) as u8;
            dib.extend_from_slice(&[over_white(px[2]), over_white(px[1]), over_white(px[0]), 255]);
        }
    }
    Some(dib)
}

/// UTF-8 HTML → Windows 剪贴板的 `HTML Format`：带一段写明各部分位置的文字头。
///
/// 已经用 `<!--StartFragment-->`、`<!--EndFragment-->` 标出片段时照用；
/// 是完整文档时把 `<body>` 里的内容当作片段；否则把它包进一个最小的文档里。
/// 返回的数据不带结尾的 0。
pub fn html_to_windows(html: &str) -> Vec<u8> {
    const START: &str = "<!--StartFragment-->";
    const END: &str = "<!--EndFragment-->";
    let lower = html.to_ascii_lowercase();
    let body = match (
        lower.find(&START.to_ascii_lowercase()),
        lower.rfind(&END.to_ascii_lowercase()),
    ) {
        (Some(start), Some(end)) if start + START.len() <= end => html.to_owned(),
        _ => match body_range(&lower) {
            Some((open_end, close)) => format!(
                "{}{START}{}{END}{}",
                &html[..open_end],
                &html[open_end..close],
                &html[close..]
            ),
            None => format!("<html><body>{START}{html}{END}</body></html>"),
        },
    };
    let lower = body.to_ascii_lowercase();
    let fragment_start = lower
        .find(&START.to_ascii_lowercase())
        .map_or(0, |i| i + START.len());
    let fragment_end = lower.rfind(&END.to_ascii_lowercase()).unwrap_or(body.len());

    // 各个位置都写成 10 位数，文字头的长度因此是固定的。
    let header_len = header(0, 0, 0, 0).len();
    let at = |i: usize| header_len + i;
    let mut out = header(at(0), at(body.len()), at(fragment_start), at(fragment_end)).into_bytes();
    out.extend_from_slice(body.as_bytes());
    out
}

/// Windows 剪贴板的 `HTML Format` → UTF-8 HTML：去掉文字头，取 `StartHTML` 到 `EndHTML` 的那一段。
/// 文字头不完整时返回 `None`。
pub fn html_from_windows(data: &[u8]) -> Option<String> {
    let end_of_data = data.iter().position(|&b| b == 0).unwrap_or(data.len());
    let data = &data[..end_of_data];
    let field = |name: &str| -> Option<i64> {
        let text = std::str::from_utf8(&data[..data.len().min(512)])
            .ok()
            .or_else(|| {
                // 文字头之后可能就是多字节字符，截在了半个字符上；文字头本身总是 ASCII。
                let ascii = data.iter().take(512).take_while(|b| b.is_ascii()).count();
                std::str::from_utf8(&data[..ascii]).ok()
            })?;
        let line = text.lines().find(|l| l.starts_with(name))?;
        line[name.len()..].trim().parse().ok()
    };
    let (start, end) = match (field("StartHTML:"), field("EndHTML:")) {
        (Some(s), Some(e)) if s >= 0 && e >= 0 => (s, e),
        // 只给了片段的位置（StartHTML 为 -1 或缺失）。
        _ => (field("StartFragment:")?, field("EndFragment:")?),
    };
    let start = usize::try_from(start).ok()?.min(data.len());
    let end = usize::try_from(end).ok()?.min(data.len());
    (start <= end).then(|| String::from_utf8_lossy(&data[start..end]).into_owned())
}

fn header(
    start_html: usize,
    end_html: usize,
    start_fragment: usize,
    end_fragment: usize,
) -> String {
    format!(
        "Version:0.9\r\nStartHTML:{start_html:010}\r\nEndHTML:{end_html:010}\r\n\
         StartFragment:{start_fragment:010}\r\nEndFragment:{end_fragment:010}\r\n"
    )
}

/// 完整文档里 `<body …>` 结束的位置和 `</body>` 开始的位置。`lower` 是转成小写的文档。
fn body_range(lower: &str) -> Option<(usize, usize)> {
    let open = lower.find("<body")?;
    let open_end = open + lower[open..].find('>')? + 1;
    let close = lower.rfind("</body")?;
    (open_end <= close).then_some((open_end, close))
}

fn decode_png(png: &[u8]) -> Option<Rgba> {
    let mut decoder = png::Decoder::new(Cursor::new(png));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let (width, height) = {
        let info = reader.info();
        (info.width, info.height)
    };
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return None;
    }
    let mut buf = vec![0; reader.output_buffer_size()?];
    let frame = reader.next_frame(&mut buf).ok()?;
    buf.truncate(frame.buffer_size());
    let pixels = match frame.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => buf
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        // 调色板图片在 normalize_to_color8 之后已经展开成 RGB 或 RGBA。
        png::ColorType::Indexed => return None,
    };
    (pixels.len() == width as usize * height as usize * 4).then_some(Rgba {
        width,
        height,
        pixels,
    })
}

fn encode_png(image: &Rgba) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, image.width, image.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        // 剪贴板里的图片要尽快交出去，压得快比压得小要紧。
        encoder.set_compression(png::Compression::Fast);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(&image.pixels).ok()?;
    }
    Some(out)
}

fn decode_dib(dib: &[u8]) -> Option<Rgba> {
    let u16_at = |at: usize| Some(u16::from_le_bytes(dib.get(at..at + 2)?.try_into().ok()?));
    let u32_at = |at: usize| Some(u32::from_le_bytes(dib.get(at..at + 4)?.try_into().ok()?));
    let i32_at = |at: usize| Some(i32::from_le_bytes(dib.get(at..at + 4)?.try_into().ok()?));

    let header = u32_at(0)? as usize;
    if header < INFO_HEADER {
        return None;
    }
    let width = i32_at(4)?;
    let height = i32_at(8)?;
    let bits = u16_at(14)?;
    let compression = u32_at(16)?;
    let colors = u32_at(32)? as usize;
    if width <= 0 || height == 0 || !matches!(bits, 24 | 32) {
        return None;
    }
    let top_down = height < 0;
    let (width, height) = (width as u32, height.unsigned_abs());
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return None;
    }

    // 颜色掩码：BI_BITFIELDS 时紧跟在 40 字节的信息头后面，更新的信息头里自带。
    let mut offset = header;
    let masks = match compression {
        BI_RGB => None,
        BI_BITFIELDS | BI_ALPHABITFIELDS if bits == 32 => {
            let at = INFO_HEADER;
            let alpha = if compression == BI_ALPHABITFIELDS || header > INFO_HEADER {
                u32_at(at + 12).unwrap_or(0)
            } else {
                0
            };
            if header == INFO_HEADER {
                offset += if compression == BI_ALPHABITFIELDS {
                    16
                } else {
                    12
                };
            }
            Some([u32_at(at)?, u32_at(at + 4)?, u32_at(at + 8)?, alpha])
        }
        _ => return None,
    };
    offset += colors * 4;

    let stride = (width as usize * usize::from(bits)).div_ceil(32) * 4;
    let pixels_len = stride.checked_mul(height as usize)?;
    let data = dib.get(offset..offset.checked_add(pixels_len)?)?;

    let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
    let mut any_alpha = false;
    for y in 0..height as usize {
        let src = if top_down { y } else { height as usize - 1 - y };
        let row = &data[src * stride..src * stride + stride];
        for x in 0..width as usize {
            let px = match (bits, masks) {
                (24, _) => {
                    let p = &row[x * 3..x * 3 + 3];
                    [p[2], p[1], p[0], 255]
                }
                (_, None) => {
                    let p = &row[x * 4..x * 4 + 4];
                    [p[2], p[1], p[0], p[3]]
                }
                (_, Some([r, g, b, a])) => {
                    let v = u32::from_le_bytes(row[x * 4..x * 4 + 4].try_into().ok()?);
                    [
                        channel(v, r),
                        channel(v, g),
                        channel(v, b),
                        if a == 0 { 255 } else { channel(v, a) },
                    ]
                }
            };
            any_alpha |= px[3] != 0;
            pixels.extend_from_slice(&px);
        }
    }
    // 32 位的透明度全为 0：这个程序没有填透明度，按不透明处理。
    if bits == 32 && !any_alpha {
        for px in pixels.chunks_exact_mut(4) {
            px[3] = 255;
        }
    }
    Some(Rgba {
        width,
        height,
        pixels,
    })
}

/// 按掩码取出一个颜色分量，换算成 8 位。
fn channel(value: u32, mask: u32) -> u8 {
    if mask == 0 {
        return 0;
    }
    let shift = mask.trailing_zeros();
    let max = u64::from(mask >> shift);
    let v = u64::from((value & mask) >> shift);
    ((v * 255 + max / 2) / max) as u8
}
