use ferry_core::clip_format::{
    dib_to_png, html_from_windows, html_to_windows, png_size, png_to_dib,
};

/// 编码一张 RGBA 图片。
fn png_of(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(rgba)
        .unwrap();
    out
}

/// 解码成逐行从上到下的 RGBA。
fn rgba_of(png: &[u8]) -> (u32, u32, Vec<u8>) {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    buf.truncate(info.buffer_size());
    assert_eq!(info.color_type, png::ColorType::Rgba);
    (info.width, info.height, buf)
}

/// 拼一个 `CF_DIB`：40 字节的信息头，后面可以跟颜色掩码，再跟像素。
fn dib(
    width: i32,
    height: i32,
    bits: u16,
    compression: u32,
    extra: &[u8],
    pixels: &[u8],
) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&width.to_le_bytes());
    out.extend_from_slice(&height.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&bits.to_le_bytes());
    out.extend_from_slice(&compression.to_le_bytes());
    out.extend_from_slice(&(pixels.len() as u32).to_le_bytes());
    out.extend_from_slice(&[0; 16]);
    out.extend_from_slice(extra);
    out.extend_from_slice(pixels);
    out
}

/// 2 × 2：上面一行红、绿，下面一行蓝、白。
const TOP_LEFT_RED: [u8; 16] = [
    255, 0, 0, 255, 0, 255, 0, 255, //
    0, 0, 255, 255, 255, 255, 255, 255,
];

#[test]
fn png_size_reads_the_header_only() {
    let png = png_of(2, 2, &TOP_LEFT_RED);
    assert_eq!(png_size(&png), Some((2, 2)));
    assert_eq!(png_size(b"GIF89a, not a png at all"), None);
    assert_eq!(png_size(&png[..10]), None);
}

#[test]
fn a_bottom_up_24_bit_bitmap_becomes_the_same_picture() {
    // 自下而上：先是下面一行（蓝、白），再是上面一行（红、绿）。每行 6 字节，补到 8 字节。
    let pixels = [
        255, 0, 0, 255, 255, 255, 0, 0, // 蓝、白（BGR）
        0, 0, 255, 0, 255, 0, 0, 0, // 红、绿
    ];
    let png = dib_to_png(&dib(2, 2, 24, 0, &[], &pixels)).unwrap();
    assert_eq!(rgba_of(&png), (2, 2, TOP_LEFT_RED.to_vec()));
}

#[test]
fn a_top_down_32_bit_bitmap_without_alpha_is_opaque() {
    // 高度为负：自上而下。第四个字节全是 0，程序没填透明度。
    let pixels = [
        0, 0, 255, 0, 0, 255, 0, 0, // 红、绿（BGRA）
        255, 0, 0, 0, 255, 255, 255, 0, // 蓝、白
    ];
    let png = dib_to_png(&dib(2, -2, 32, 0, &[], &pixels)).unwrap();
    assert_eq!(rgba_of(&png), (2, 2, TOP_LEFT_RED.to_vec()));
}

#[test]
fn a_bitfields_bitmap_reads_its_masks() {
    let masks: Vec<u8> = [0x00FF_0000u32, 0x0000_FF00, 0x0000_00FF]
        .iter()
        .flat_map(|m| m.to_le_bytes())
        .collect();
    let pixels = [
        0, 0, 255, 0, 0, 255, 0, 0, //
        255, 0, 0, 0, 255, 255, 255, 0,
    ];
    let png = dib_to_png(&dib(2, -2, 32, 3, &masks, &pixels)).unwrap();
    assert_eq!(rgba_of(&png), (2, 2, TOP_LEFT_RED.to_vec()));
}

#[test]
fn unsupported_bitmaps_are_skipped() {
    assert_eq!(
        dib_to_png(&dib(2, 2, 8, 0, &[], &[0; 8])),
        None,
        "调色板位图"
    );
    assert_eq!(
        dib_to_png(&dib(2, 2, 24, 0, &[], &[0; 4])),
        None,
        "像素不够"
    );
    assert_eq!(dib_to_png(b"short"), None);
}

#[test]
fn an_opaque_png_round_trips_through_a_bitmap() {
    let png = png_of(2, 2, &TOP_LEFT_RED);
    let dib = png_to_dib(&png).unwrap();
    assert_eq!(dib.len(), 40 + 2 * 2 * 4);
    assert_eq!(
        i32::from_le_bytes(dib[8..12].try_into().unwrap()),
        2,
        "自下而上"
    );
    let back = dib_to_png(&dib).unwrap();
    assert_eq!(rgba_of(&back), (2, 2, TOP_LEFT_RED.to_vec()));
}

#[test]
fn transparent_pixels_land_on_white_in_the_bitmap() {
    // 半透明的黑和全透明的红。
    let png = png_of(2, 1, &[0, 0, 0, 128, 255, 0, 0, 0]);
    let dib = png_to_dib(&png).unwrap();
    let pixels = &dib[40..];
    assert_eq!(
        pixels[..4],
        [127, 127, 127, 255],
        "半透明的黑叠在白底上是灰"
    );
    assert_eq!(pixels[4..8], [255, 255, 255, 255], "全透明就是白底");
}

#[test]
fn html_fragments_are_wrapped_with_correct_offsets() {
    let data = html_to_windows("<b>你好</b>");
    let text = String::from_utf8(data.clone()).unwrap();
    assert!(text.starts_with("Version:0.9\r\n"));
    let at = |name: &str| -> usize {
        let line = text.lines().find(|l| l.starts_with(name)).unwrap();
        line[name.len()..].parse().unwrap()
    };
    assert_eq!(
        &data[at("StartFragment:")..at("EndFragment:")],
        "<b>你好</b>".as_bytes()
    );
    assert_eq!(at("EndHTML:"), data.len());
    assert!(data[at("StartHTML:")..].starts_with(b"<html><body>"));
}

#[test]
fn a_full_document_uses_its_body_as_the_fragment() {
    let doc = "<html><head><meta charset=\"utf-8\"></head><BODY class=x><p>段落</p></BODY></html>";
    let data = html_to_windows(doc);
    let back = html_from_windows(&data).unwrap();
    assert!(
        back.contains("<!--StartFragment--><p>段落</p><!--EndFragment-->"),
        "{back}"
    );
    assert!(back.starts_with("<html><head>"));
}

#[test]
fn html_round_trips_through_the_windows_format() {
    for html in ["纯文字", "<i>斜体</i>", "<html><body>x</body></html>"] {
        let back = html_from_windows(&html_to_windows(html)).unwrap();
        assert!(
            back.contains(
                html.trim_start_matches("<html><body>")
                    .trim_end_matches("</body></html>")
            )
        );
    }
}

#[test]
fn html_from_windows_falls_back_to_the_fragment_and_ignores_the_trailing_zero() {
    let body = "<p>x</p>";
    let header = "Version:1.0\r\nStartHTML:-1\r\nEndHTML:-1\r\nStartFragment:0000000089\r\nEndFragment:0000000097\r\n";
    assert_eq!(header.len(), 89);
    let mut data = format!("{header}{body}").into_bytes();
    data.push(0);
    assert_eq!(html_from_windows(&data).unwrap(), body);
    assert_eq!(html_from_windows(b"no header here"), None);
}
