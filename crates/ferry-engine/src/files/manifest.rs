//! 发送方：把用户给的路径展开成清单。

use std::collections::HashSet;
use std::fs::{self, Metadata};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use ferry_core::file_name::{MAX_DEPTH, clean_name, unique};
use ferry_proto::limits::MAX_PATH_LEN;
use ferry_proto::{EntryKind, FileEntry, FileOffer};
use ferry_types::TransferId;

/// 一次最多发送的条目数（文件加文件夹）。
pub(crate) const MAX_ENTRIES: usize = 100_000;

/// 系统自己生成的文件，对方用不上，不发送。
const SKIPPED: [&str; 3] = [".DS_Store", "Thumbs.db", "desktop.ini"];

/// 一次发送的完整清单。
pub(crate) struct Plan {
    pub(crate) id: TransferId,
    /// 第一个顶层项目的名字。
    pub(crate) name: String,
    pub(crate) items: u32,
    pub(crate) files: u32,
    pub(crate) bytes: u64,
    /// 按发送顺序排列，文件夹在它包含的条目之前。
    pub(crate) entries: Vec<FileEntry>,
    /// 与 `entries` 一一对应的本机路径。
    pub(crate) sources: Vec<PathBuf>,
}

impl Plan {
    pub(crate) fn offer(&self) -> FileOffer {
        FileOffer {
            id: self.id,
            name: self.name.clone(),
            items: self.items,
            files: self.files,
            bytes: self.bytes,
        }
    }
}

/// 展开 `paths`：文件原样，文件夹递归展开。
///
/// 子项按名字排序，同样的内容两次展开得到同样的清单，续传才对得上。
/// 名字按两个系统都能用的规则清洗，同一层里清洗后重名（不分大小写）的自动编号。
/// 顶层的符号链接跟随一次，文件夹里的符号链接跳过，避免循环和意外带走别处的文件。
pub(crate) fn build(id: TransferId, paths: &[PathBuf]) -> Result<Plan, String> {
    let mut plan = Plan {
        id,
        name: String::new(),
        items: 0,
        files: 0,
        bytes: 0,
        entries: Vec::new(),
        sources: Vec::new(),
    };
    let mut taken = HashSet::new();
    for path in paths {
        let raw = path
            .file_name()
            .ok_or_else(|| format!("不能发送整个磁盘：{}", path.display()))?
            .to_string_lossy();
        let meta = fs::metadata(path).map_err(|e| format!("找不到 {}：{e}", path.display()))?;
        let name = unique(&clean_name(&raw), |n| taken.contains(&n.to_lowercase()));
        taken.insert(name.to_lowercase());
        let before = plan.entries.len();
        add(&mut plan, path, &meta, name.clone(), 1)?;
        if plan.entries.len() > before {
            plan.items += 1;
            if plan.name.is_empty() {
                plan.name = name;
            }
        }
    }
    if plan.entries.is_empty() {
        return Err("没有可以发送的文件".into());
    }
    Ok(plan)
}

fn add(
    plan: &mut Plan,
    source: &Path,
    meta: &Metadata,
    rel: String,
    depth: usize,
) -> Result<(), String> {
    if plan.entries.len() >= MAX_ENTRIES {
        return Err(format!("一次最多发送 {MAX_ENTRIES} 个文件和文件夹"));
    }
    if rel.len() > MAX_PATH_LEN || depth > MAX_DEPTH {
        return Err(format!("路径太长或层级太深：{rel}"));
    }
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));

    if meta.is_dir() {
        plan.entries.push(FileEntry {
            path: rel.clone(),
            kind: EntryKind::Dir,
            size: 0,
            modified,
        });
        plan.sources.push(source.to_owned());

        let mut children: Vec<_> = fs::read_dir(source)
            .map_err(|e| format!("读不了文件夹 {}：{e}", source.display()))?
            .filter_map(|entry| entry.ok().map(|e| e.file_name()))
            .collect();
        children.sort();
        let mut taken = HashSet::new();
        for child in children {
            let raw = child.to_string_lossy();
            if SKIPPED.contains(&raw.as_ref()) {
                continue;
            }
            let path = source.join(&child);
            let Ok(meta) = fs::symlink_metadata(&path) else {
                continue;
            };
            if meta.file_type().is_symlink() {
                continue;
            }
            let name = unique(&clean_name(&raw), |n| taken.contains(&n.to_lowercase()));
            taken.insert(name.to_lowercase());
            add(plan, &path, &meta, format!("{rel}/{name}"), depth + 1)?;
        }
    } else if meta.is_file() {
        plan.entries.push(FileEntry {
            path: rel,
            kind: EntryKind::File,
            size: meta.len(),
            modified,
        });
        plan.sources.push(source.to_owned());
        plan.files += 1;
        plan.bytes += meta.len();
    }
    // 设备、管道等特殊文件跳过。
    Ok(())
}
