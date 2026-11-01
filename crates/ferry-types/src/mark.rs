//! 运行日志里的敏感内容：设备名、文件名、路径。
//!
//! 写进运行日志时用 `⟦d:Mac Studio⟧` 这样的标记括起来：设置界面上显示时去掉标记，
//! 导出诊断文件时把标记里的内容换成代号（ferry-core 的 `Redactor`，ADR 0012）。

use std::borrow::Cow;
use std::fmt::{self, Write};
use std::path::Path;

/// 标记的开头和结尾。日志里几乎不会自然出现这两个字符。
const OPEN: char = '⟦';
const CLOSE: char = '⟧';

/// 敏感内容的种类。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sensitive {
    /// 设备名：本机或对方的名字、本机给它起的名字。
    Device,
    /// 文件或文件夹的名字。
    File,
    /// 本机上的完整路径。
    Path,
}

impl Sensitive {
    fn tag(self) -> char {
        match self {
            Self::Device => 'd',
            Self::File => 'f',
            Self::Path => 'p',
        }
    }

    fn from_tag(tag: char) -> Option<Self> {
        match tag {
            'd' => Some(Self::Device),
            'f' => Some(Self::File),
            'p' => Some(Self::Path),
            _ => None,
        }
    }
}

/// 一段带标记的敏感内容，按 [`Display`](fmt::Display) 写进日志。
#[derive(Clone, Debug)]
pub struct Marked<'a> {
    kind: Sensitive,
    text: Cow<'a, str>,
}

impl fmt::Display for Marked<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_char(OPEN)?;
        f.write_char(self.kind.tag())?;
        f.write_char(':')?;
        // 内容里的结尾符换掉，免得标记提前结束。
        for c in self.text.chars() {
            f.write_char(if c == CLOSE { ')' } else { c })?;
        }
        f.write_char(CLOSE)
    }
}

/// 设备名。
pub fn device(name: &str) -> Marked<'_> {
    Marked {
        kind: Sensitive::Device,
        text: Cow::Borrowed(name),
    }
}

/// 文件或文件夹的名字。
pub fn file(name: &str) -> Marked<'_> {
    Marked {
        kind: Sensitive::File,
        text: Cow::Borrowed(name),
    }
}

/// 本机上的完整路径。
pub fn path(path: &Path) -> Marked<'_> {
    Marked {
        kind: Sensitive::Path,
        text: path.to_string_lossy(),
    }
}

/// 日志一行里的一段：普通文字，或者带标记的敏感内容。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Piece<'a> {
    Plain(&'a str),
    Marked(Sensitive, &'a str),
}

/// 把一行日志拆成普通文字和标记里的内容。不完整、不认识的标记按普通文字处理。
pub fn pieces(line: &str) -> Vec<Piece<'_>> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find(OPEN) {
        let after = &rest[start + OPEN.len_utf8()..];
        let mut chars = after.chars();
        let marked = match (chars.next().and_then(Sensitive::from_tag), chars.next()) {
            (Some(kind), Some(':')) => {
                let body = &after[2..];
                body.find(CLOSE)
                    .map(|end| (kind, &body[..end], &body[end + CLOSE.len_utf8()..]))
            }
            _ => None,
        };
        match marked {
            Some((kind, text, tail)) => {
                if start > 0 {
                    out.push(Piece::Plain(&rest[..start]));
                }
                out.push(Piece::Marked(kind, text));
                rest = tail;
            }
            None => {
                // 不是标记：开头符当普通文字，从它后面接着找。
                let next = start + OPEN.len_utf8();
                out.push(Piece::Plain(&rest[..next]));
                rest = &rest[next..];
            }
        }
    }
    if !rest.is_empty() {
        out.push(Piece::Plain(rest));
    }
    out
}

/// 去掉标记，只留内容：设置界面上显示的样子。
pub fn strip(line: &str) -> Cow<'_, str> {
    if !line.contains(OPEN) {
        return Cow::Borrowed(line);
    }
    let mut out = String::with_capacity(line.len());
    for piece in pieces(line) {
        match piece {
            Piece::Plain(text) | Piece::Marked(_, text) => out.push_str(text),
        }
    }
    Cow::Owned(out)
}
