//! 运行日志与诊断文件（ADR 0012）。
//!
//! - 读运行日志给设置界面看：第一次取最后一段，之后只取新写的；去掉敏感内容的标记，照原样显示。
//! - 诊断文件：开头是版本、系统、设备和设置，后面是两卷运行日志；摘要只带最后 200 行，能直接贴进问题描述。
//!   设备名、文件名和用户目录换成代号（ferry-core 的 `Redactor`），局域网地址留着。
//!   以前的版本写的行没有标记、换不干净，不带上。

use std::fs;
use std::path::Path;

use ferry_core::Redactor;
use ferry_ipc::{RunLogLevel, RunLogLine};
use ferry_types::mark;

/// 第一次读运行日志时最多取最后这么多行。
const TAIL_LINES: usize = 500;
/// 一次最多读这么多字节。新写的超过这么多时只取最后这一段，界面整个替换。
const MAX_CHUNK: usize = 256 * 1024;
/// 摘要里运行日志的行数：GitHub 的问题描述最多 6.5 万字，要贴得下。
const SUMMARY_LINES: usize = 200;

/// 读到的一段运行日志。
pub(crate) struct Chunk {
    pub(crate) lines: Vec<RunLogLine>,
    /// 下次从这里接着读。
    pub(crate) next: u64,
    /// 界面要整个替换，而不是接在后面。
    pub(crate) fresh: bool,
    /// 当前这卷的大小和行数。
    pub(crate) bytes: u64,
    pub(crate) lines_total: u64,
}

/// 读运行日志 `path`：`since` 为空时取最后 [`TAIL_LINES`] 行，否则取 `since` 之后新写的。
/// 日志换了一卷（文件比 `since` 短）或者新写的太多时，从头取最后一段。
pub(crate) fn read(path: &Path, since: Option<u64>) -> Chunk {
    let data = fs::read(path).unwrap_or_default();
    let bytes = data.len();
    let (start, fresh) = match since.and_then(|s| usize::try_from(s).ok()) {
        Some(s) if s <= bytes && bytes - s <= MAX_CHUNK => (s, false),
        _ => (bytes.saturating_sub(MAX_CHUNK), true),
    };
    // 只要完整的行：从中间开始时跳过第一行的后半截，最后没写完的一行留到下次。
    let mut from = start;
    if fresh && start > 0 {
        from = data[start..]
            .iter()
            .position(|&b| b == b'\n')
            .map_or(bytes, |i| start + i + 1);
    }
    let end = data[from..]
        .iter()
        .rposition(|&b| b == b'\n')
        .map_or(from, |i| from + i + 1);
    let text = String::from_utf8_lossy(&data[from..end]);
    let mut lines: Vec<RunLogLine> = text.lines().filter(|l| !l.is_empty()).map(parse).collect();
    if fresh && lines.len() > TAIL_LINES {
        lines.drain(..lines.len() - TAIL_LINES);
    }
    Chunk {
        lines,
        next: end as u64,
        fresh,
        bytes: bytes as u64,
        lines_total: data.iter().filter(|&&b| b == b'\n').count() as u64,
    }
}

/// 解析运行日志的一行，去掉敏感内容的标记。
///
/// 现在的格式是 `2026-11-02 06:12:03.104  INFO 内容`；以前的版本写的是启动后的秒数
/// （`   0.096268792s  INFO 内容`），没有时间。
fn parse(line: &str) -> RunLogLine {
    let (at, rest) = match line.get(..23) {
        Some(stamp) if is_stamp(stamp) => (stamp[11..19].to_owned(), &line[23..]),
        _ => {
            let rest = line.trim_start();
            let rest = match rest.split_once(' ') {
                Some((uptime, tail)) if is_uptime(uptime) => tail,
                _ => rest,
            };
            (String::new(), rest)
        }
    };
    let rest = rest.trim_start();
    let (level, text) = match rest.split_once(' ') {
        Some(("INFO", text)) => (RunLogLevel::Info, text),
        Some(("WARN", text)) => (RunLogLevel::Warn, text),
        Some(("ERROR", text)) => (RunLogLevel::Error, text),
        Some(("DEBUG" | "TRACE", text)) => (RunLogLevel::Debug, text),
        _ => (RunLogLevel::Info, rest),
    };
    RunLogLine {
        at,
        level,
        text: mark::strip(text.trim_start()).into_owned(),
    }
}

/// `2026-11-02 06:12:03.104`
fn is_stamp(text: &str) -> bool {
    let b = text.as_bytes();
    b.len() == 23
        && b.iter().enumerate().all(|(i, &c)| match i {
            4 | 7 => c == b'-',
            10 => c == b' ',
            13 | 16 => c == b':',
            19 => c == b'.',
            _ => c.is_ascii_digit(),
        })
}

/// `0.096268792s`
fn is_uptime(text: &str) -> bool {
    text.strip_suffix('s')
        .is_some_and(|n| !n.is_empty() && n.parse::<f64>().is_ok())
}

/// 诊断文件或摘要：`header` 是开头的若干行（已经换好代号），后面跟运行日志。
/// `logs` 从旧到新；`summary` 为真时只带最后 [`SUMMARY_LINES`] 行。
pub(crate) fn report(
    title: &str,
    header: &[(&str, String)],
    logs: &[&Path],
    redactor: &mut Redactor,
    summary: bool,
) -> String {
    let mut out = format!("{title}\n\n");
    let width = header
        .iter()
        .map(|(label, _)| label.chars().count())
        .max()
        .unwrap_or(0);
    // 标签是汉字，用全角空格补齐，等宽字体里才对得齐。
    for (label, value) in header {
        let pad = "\u{3000}".repeat(width - label.chars().count());
        let mut rows = value.lines();
        out.push_str(&format!("{label}{pad}  {}\n", rows.next().unwrap_or("")));
        for row in rows {
            out.push_str(&format!("{}  {row}\n", "\u{3000}".repeat(width)));
        }
    }

    let mut text = String::new();
    let mut names = Vec::new();
    for path in logs {
        if let Ok(data) = fs::read(path) {
            text.push_str(&String::from_utf8_lossy(&data));
            if let Some(name) = path.file_name() {
                names.push(name.to_string_lossy().into_owned());
            }
        }
    }
    // 以前的版本写的行没有时间、也没有给设备名和文件名加标记，换不干净，不带上。
    let all = text.lines().filter(|l| !l.is_empty());
    let (lines, old): (Vec<&str>, Vec<&str>) = all.partition(|l| l.get(..23).is_some_and(is_stamp));
    let shown = if summary {
        &lines[lines.len().saturating_sub(SUMMARY_LINES)..]
    } else {
        &lines[..]
    };
    let source = if names.is_empty() {
        "没有找到运行日志".to_owned()
    } else {
        names.join("、")
    };
    out.push_str(&format!(
        "\n—— 运行日志：{source}，最后 {} 行 ——\n",
        shown.len()
    ));
    if !old.is_empty() {
        out.push_str(&format!(
            "（以前的版本写的 {} 行没有带上：它们没有给设备名、文件名加标记）\n",
            old.len()
        ));
    }
    for line in shown {
        out.push_str(&redactor.line(line));
        out.push('\n');
    }
    out
}

/// Unix 毫秒加上时区，写成本机时间：`(2026-11-02 06:24:13 +08:00, 20261102-0624)`。
pub(crate) fn local_time(unix_ms: i64, offset_secs: i32) -> (String, String) {
    let secs = unix_ms.div_euclid(1000) + i64::from(offset_secs);
    let (days, rest) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let (y, m, d) = civil_from_days(days);
    let (hh, mm, ss) = (rest / 3600, rest % 3600 / 60, rest % 60);
    let sign = if offset_secs < 0 { '-' } else { '+' };
    let off = offset_secs.unsigned_abs();
    (
        format!(
            "{y:04}-{m:02}-{d:02} {hh:02}:{mm:02}:{ss:02} {sign}{:02}:{:02}",
            off / 3600,
            off % 3600 / 60
        ),
        format!("{y:04}{m:02}{d:02}-{hh:02}{mm:02}"),
    )
}

/// 1970-01-01 之后的天数换成年月日（公历）。
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}
