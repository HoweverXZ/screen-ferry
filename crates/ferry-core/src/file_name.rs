//! 收到的文件名与路径：按两个系统都能接受的规则清洗，重名时自动编号。
//!
//! 对方给的名字不可信。规则在 macOS 和 Windows 上一致，同一个文件发到哪边都叫同一个名字。

/// 清单中路径的最大层数。
pub const MAX_DEPTH: usize = 64;

/// 单个名字的最大字节数（UTF-8）。两个系统的文件名上限都是 255。
pub const MAX_NAME_BYTES: usize = 255;

/// Windows 保留的设备名，不区分大小写，带不带扩展名都不能用作文件名。
const RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// 把一个名字清洗成两个系统都能用的文件名：
///
/// - 控制字符和 `< > : " / \ | ? *` 换成 `_`；
/// - 去掉结尾的空格和点（Windows 会悄悄去掉它们，造成重名）；
/// - Windows 保留的设备名前面加 `_`；
/// - 超过 255 字节时截短，尽量保留扩展名；
/// - 清洗后为空时用 `_`。
pub fn clean_name(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let kept = out.trim_end_matches([' ', '.']).len();
    out.truncate(kept);
    if out.is_empty() {
        return "_".into();
    }
    let stem = out.split('.').next().unwrap_or_default();
    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(stem)) {
        out.insert(0, '_');
    }
    shorten(&out)
}

/// 校验并清洗清单中的相对路径（以 `/` 分隔）。
///
/// 绝对路径、空段、`.`、`..`、超过 [`MAX_DEPTH`] 层都视为非法，返回 `None`；
/// 其余每一段按 [`clean_name`] 清洗。
pub fn clean_path(path: &str) -> Option<Vec<String>> {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() > MAX_DEPTH {
        return None;
    }
    parts
        .into_iter()
        .map(|part| match part {
            "" | "." | ".." => None,
            part => Some(clean_name(part)),
        })
        .collect()
}

/// 第 `n` 个同名文件的名字：`报告.pdf` → `报告 (2).pdf`，`设计稿` → `设计稿 (2)`。
///
/// 只有一个点且在开头的名字（`.env`）没有扩展名。
pub fn numbered(name: &str, n: u32) -> String {
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => name.split_at(i),
        _ => (name, ""),
    };
    shorten(&format!("{stem} ({n}){ext}"))
}

/// 在 `taken` 为真的名字之外，挑一个不重名的：先试原名，再试 `(2)`、`(3)`……
pub fn unique(name: &str, mut taken: impl FnMut(&str) -> bool) -> String {
    if !taken(name) {
        return name.to_owned();
    }
    (2..)
        .map(|n| numbered(name, n))
        .find(|candidate| !taken(candidate))
        .expect("编号用不完")
}

/// 超过 [`MAX_NAME_BYTES`] 时截短名字，尽量保留不长的扩展名。
fn shorten(name: &str) -> String {
    if name.len() <= MAX_NAME_BYTES {
        return name.to_owned();
    }
    let ext = match name.rfind('.') {
        Some(i) if i > 0 && name.len() - i <= 16 => &name[i..],
        _ => "",
    };
    let mut stem_len = MAX_NAME_BYTES - ext.len();
    while !name.is_char_boundary(stem_len) {
        stem_len -= 1;
    }
    format!("{}{ext}", &name[..stem_len])
}
