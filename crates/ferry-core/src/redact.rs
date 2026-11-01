//! 导出诊断文件时把运行日志里的敏感内容换成代号（ADR 0012）。
//!
//! 换带标记的内容（见 `ferry_types::mark`）和用户目录：本机的名字换成“本机”，其他设备依次换成
//! “设备 A”“设备 B”……，文件名换成“文件 1.p12”这样只留扩展名，用户目录换成 `~`。
//! 没带标记的正文里（例如错误信息）再把已知设备的名字换掉，以防漏网；“Windows”“Mac”这类通用的名字
//! 不换，它们本来也不暴露什么，换了反而读不懂。
//! 同一个名字在整份文件里始终换成同一个代号。局域网地址不换，排查连接问题要用。

use std::collections::HashMap;

use ferry_types::mark::{self, Piece, Sensitive};

#[derive(Debug, Default)]
pub struct Redactor {
    /// 名字 → 代号。本机和已知设备的几个名字事先登记，日志里遇到的其他设备按出现顺序补上。
    devices: HashMap<String, String>,
    next_device: usize,
    files: HashMap<String, String>,
    home: Option<String>,
}

impl Redactor {
    /// `home` 是用户目录，日志里出现的地方都换成 `~`。
    pub fn new(home: Option<&str>) -> Self {
        Self {
            home: home.filter(|h| h.len() > 1).map(str::to_owned),
            ..Self::default()
        }
    }

    /// 本机的名字换成“本机”。
    pub fn local(&mut self, name: &str) {
        self.devices.insert(name.to_owned(), "本机".to_owned());
    }

    /// 一台设备的几个名字（它自己报的名字、本机给它起的名字）换成同一个代号，返回这个代号。
    /// 已经登记过其中某个名字时沿用它的代号。
    pub fn device(&mut self, names: &[&str]) -> String {
        let known = names.iter().find_map(|n| self.devices.get(*n)).cloned();
        let code = known.unwrap_or_else(|| self.next_device_code());
        for name in names.iter().filter(|n| !n.is_empty()) {
            self.devices.insert((*name).to_owned(), code.clone());
        }
        code
    }

    /// 换掉一行里带标记的内容和用户目录。
    pub fn line(&mut self, line: &str) -> String {
        let mut out = String::with_capacity(line.len());
        for piece in mark::pieces(line) {
            match piece {
                Piece::Plain(text) => {
                    let text = self.unhome(text);
                    out.push_str(&self.unname(&text));
                }
                Piece::Marked(Sensitive::Device, name) => {
                    let code = self.device(&[name]);
                    out.push_str(&code);
                }
                Piece::Marked(Sensitive::File, name) => {
                    let code = self.file(name);
                    out.push_str(&code);
                }
                Piece::Marked(Sensitive::Path, path) => {
                    let path = self.unhome(path);
                    // 只换最后一段（文件名）；目录留着，看得出放在哪里。
                    match path.rfind(['/', '\\']) {
                        Some(i) if i + 1 < path.len() => {
                            out.push_str(&path[..=i]);
                            let code = self.file(&path[i + 1..]);
                            out.push_str(&code);
                        }
                        _ => out.push_str(&path),
                    }
                }
            }
        }
        out
    }

    fn file(&mut self, name: &str) -> String {
        if let Some(code) = self.files.get(name) {
            return code.clone();
        }
        let code = format!("文件 {}{}", self.files.len() + 1, extension(name));
        self.files.insert(name.to_owned(), code.clone());
        code
    }

    /// 正文里已知设备的名字换成代号，长的名字先换。
    fn unname(&self, text: &str) -> String {
        let mut names: Vec<(&String, &String)> = self
            .devices
            .iter()
            .filter(|(name, _)| specific(name))
            .collect();
        names.sort_by_key(|(name, _)| std::cmp::Reverse(name.len()));
        let mut out = text.to_owned();
        for (name, code) in names {
            if out.contains(name.as_str()) {
                out = out.replace(name.as_str(), code);
            }
        }
        out
    }

    fn unhome(&self, text: &str) -> String {
        match &self.home {
            Some(home) => text.replace(home.as_str(), "~"),
            None => text.to_owned(),
        }
    }

    fn next_device_code(&mut self) -> String {
        let n = self.next_device;
        self.next_device += 1;
        // A–Z 之后用数字。
        match u8::try_from(n).ok().filter(|&n| n < 26) {
            Some(n) => format!("设备 {}", char::from(b'A' + n)),
            None => format!("设备 {}", n + 1),
        }
    }
}

/// 值得在正文里换掉的设备名：至少 3 个字，也不是“Windows”“Mac”这类通用的名字。
fn specific(name: &str) -> bool {
    const GENERIC: [&str; 8] = [
        "windows",
        "mac",
        "macos",
        "pc",
        "linux",
        "imac",
        "macbook",
        "screenferry",
    ];
    name.chars().count() >= 3 && !GENERIC.contains(&name.to_lowercase().as_str())
}

/// 文件名的扩展名（含点），例如 `.p12`；没有、太长或者不像扩展名时为空。
fn extension(name: &str) -> &str {
    match name.rfind('.') {
        Some(i) if i > 0 => {
            let ext = &name[i..];
            let plain = ext[1..].chars().all(|c| c.is_ascii_alphanumeric());
            if plain && (2..=11).contains(&ext.len()) {
                ext
            } else {
                ""
            }
        }
        _ => "",
    }
}
