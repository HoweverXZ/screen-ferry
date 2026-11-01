//! 接收方在文件连接上的流程：收清单 → 算续传点 → 边收边写、逐个校验 → 放到下载文件夹。
//!
//! 收到的内容先写进 `partial/<传输号>/data/`，每校验完一个文件就把进度记进 `state`。
//! 全部收好后才把顶层项目移到下载文件夹，用户不会看到写了一半的文件。

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use ferry_core::file_name::{clean_path, unique};
use ferry_net::Framed;
use ferry_proto::limits::MAX_CHUNK_LEN;
use ferry_proto::{EntryKind, FileEntry, FileOffer, FileResume, Message};
use ring::digest::{Context, SHA256};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::manifest::MAX_ENTRIES;
use super::send::recv;
use super::{Progress, Stop};

/// 两条清单消息、两段内容之间最多等多久。
const WAIT: Duration = Duration::from_secs(60);

/// 进度文件的格式标记。
const STATE_TAG: &str = "screenferry-partial-1";

pub(super) async fn run<S>(
    framed: &mut Framed<S>,
    offer: &FileOffer,
    partial: &Path,
    downloads: &Path,
    progress: &mut Progress,
) -> Result<Vec<PathBuf>, Stop>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let entries = receive_list(framed).await?;
    let (paths, tops) = check_paths(&entries)?;

    let work = partial.join(offer.id.to_string());
    let data = work.join("data");
    let fingerprint = fingerprint(&entries);
    let completed = match read_state(&work, &fingerprint).await {
        Some(completed) => completed.min(entries.len()),
        None => {
            // 没收过，或者清单变了：从头开始。
            let _ = tokio::fs::remove_dir_all(&work).await;
            0
        }
    };
    let offset = match entries.get(completed) {
        Some(entry) if entry.kind == EntryKind::File => {
            tokio::fs::metadata(data.join(&paths[completed]))
                .await
                .map_or(0, |m| m.len().min(entry.size))
        }
        _ => 0,
    };

    // 先建好所有文件夹，空文件夹也能原样带过去。
    for (entry, path) in entries.iter().zip(&paths) {
        if entry.kind == EntryKind::Dir {
            create_dir(&data.join(path)).await?;
        }
    }
    create_dir(&data).await?;
    // 收第一个文件之前就记下进度：大文件收到一半断开，下次也认得出这份清单，接着往下收。
    write_state(&work, &fingerprint, completed).await?;

    let index = u32::try_from(completed).expect("条目数有上限");
    framed
        .send(&Message::FileResume(FileResume { index, offset }))
        .await?;
    let before: u64 = entries[..completed]
        .iter()
        .filter(|e| e.kind == EntryKind::File)
        .map(|e| e.size)
        .sum();
    progress.start(before + offset);

    for i in completed..entries.len() {
        let entry = &entries[i];
        if entry.kind == EntryKind::Dir {
            continue;
        }
        let target = data.join(&paths[i]);
        if let Some(parent) = target.parent() {
            create_dir(parent).await?;
        }
        let from = if i == completed { offset } else { 0 };
        receive_file(framed, i, entry, &target, from, progress).await?;
        write_state(&work, &fingerprint, i + 1).await?;
    }

    let saved = place(data, tops, downloads.to_owned()).await?;
    let _ = tokio::fs::remove_dir_all(&work).await;
    framed.send(&Message::FileDone).await?;
    Ok(saved)
}

async fn receive_list<S>(framed: &mut Framed<S>) -> Result<Vec<FileEntry>, Stop>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut entries = Vec::new();
    loop {
        let Message::FileList(list) = recv(framed, WAIT).await? else {
            return Err(Stop::Fail("对方没有先发清单".into()));
        };
        entries.extend(list.entries);
        if entries.len() > MAX_ENTRIES {
            return Err(Stop::Fail("清单里的条目太多".into()));
        }
        if list.last {
            return Ok(entries);
        }
    }
}

/// 校验并清洗清单里的路径，返回每个条目的本机相对路径，以及按出现顺序排列的顶层名字。
fn check_paths(entries: &[FileEntry]) -> Result<(Vec<PathBuf>, Vec<String>), Stop> {
    let mut paths = Vec::with_capacity(entries.len());
    let mut tops: Vec<String> = Vec::new();
    let mut seen = HashSet::new();
    for entry in entries {
        let parts = clean_path(&entry.path)
            .ok_or_else(|| Stop::Fail(format!("清单里有非法路径：{}", entry.path)))?;
        if !seen.insert(parts.join("/").to_lowercase()) {
            return Err(Stop::Fail(format!("清单里有重复的路径：{}", entry.path)));
        }
        if !tops.contains(&parts[0]) {
            tops.push(parts[0].clone());
        }
        paths.push(parts.iter().collect());
    }
    if paths.is_empty() {
        return Err(Stop::Fail("清单是空的".into()));
    }
    Ok((paths, tops))
}

/// 收一个文件从 `from` 开始的内容，校验整个文件的 SHA-256。
async fn receive_file<S>(
    framed: &mut Framed<S>,
    index: usize,
    entry: &FileEntry,
    target: &Path,
    from: u64,
    progress: &mut Progress,
) -> Result<(), Stop>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let fail = |e: io::Error| Stop::Fail(format!("写不了 {}：{e}", entry.path));
    let mut hash = Context::new(&SHA256);
    let mut file = if from > 0 {
        // 续传：已收到的部分先读一遍算校验和，再接着往后写。
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(target)
            .await
            .map_err(fail)?;
        file.set_len(from).await.map_err(fail)?;
        let mut buf = vec![0u8; MAX_CHUNK_LEN];
        let mut read = 0;
        while read < from {
            let want = usize::try_from(from - read).map_or(buf.len(), |r| r.min(buf.len()));
            let n = file.read(&mut buf[..want]).await.map_err(fail)?;
            if n == 0 {
                return Err(Stop::Fail(format!("已收到的 {} 不完整", entry.path)));
            }
            hash.update(&buf[..n]);
            read += n as u64;
        }
        file
    } else {
        File::create(target).await.map_err(fail)?
    };

    let mut got = from;
    while got < entry.size {
        let Message::FileData { data } = recv(framed, WAIT).await? else {
            return Err(Stop::Fail(format!("{} 的内容不完整", entry.path)));
        };
        if got + data.len() as u64 > entry.size {
            return Err(Stop::Fail(format!("{} 收到的内容比说好的多", entry.path)));
        }
        hash.update(&data);
        file.write_all(&data).await.map_err(fail)?;
        got += data.len() as u64;
        progress.add(data.len() as u64);
    }
    file.flush().await.map_err(fail)?;

    let Message::FileEnd(end) = recv(framed, WAIT).await? else {
        return Err(Stop::Fail(format!("{} 没有结束标记", entry.path)));
    };
    if usize::try_from(end.index).ok() != Some(index) {
        return Err(Stop::Fail("文件顺序对不上".into()));
    }
    if hash.finish().as_ref() != end.sha256 {
        return Err(Stop::Fail(format!(
            "{} 校验不一致，内容可能损坏",
            entry.path
        )));
    }
    if entry.modified > 0 {
        let when = UNIX_EPOCH + Duration::from_millis(entry.modified);
        let file = file.into_std().await;
        // 修改时间设不上不影响内容。
        let _ = tokio::task::spawn_blocking(move || file.set_modified(when)).await;
    }
    Ok(())
}

/// 把收好的顶层项目移到下载文件夹，重名时自动编号。返回放好的位置。
async fn place(data: PathBuf, tops: Vec<String>, downloads: PathBuf) -> Result<Vec<PathBuf>, Stop> {
    tokio::task::spawn_blocking(move || {
        fs::create_dir_all(&downloads)
            .map_err(|e| Stop::Fail(format!("建不了下载文件夹 {}：{e}", downloads.display())))?;
        let mut saved = Vec::with_capacity(tops.len());
        for name in tops {
            let from = data.join(&name);
            let to = downloads.join(unique(&name, |n| downloads.join(n).exists()));
            move_item(&from, &to).map_err(|e| Stop::Fail(format!("放不进下载文件夹：{e}")))?;
            saved.push(to);
        }
        Ok(saved)
    })
    .await
    .map_err(|_| Stop::Fail("放置收到的文件时出错".into()))?
}

/// 移动文件或文件夹。不在同一块磁盘上时改为复制后删除。
fn move_item(from: &Path, to: &Path) -> io::Result<()> {
    if fs::rename(from, to).is_ok() {
        return Ok(());
    }
    copy_all(from, to)?;
    let _ = if from.is_dir() {
        fs::remove_dir_all(from)
    } else {
        fs::remove_file(from)
    };
    Ok(())
}

fn copy_all(from: &Path, to: &Path) -> io::Result<()> {
    if from.is_dir() {
        fs::create_dir_all(to)?;
        for entry in fs::read_dir(from)? {
            let entry = entry?;
            copy_all(&entry.path(), &to.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        fs::copy(from, to).map(|_| ())
    }
}

async fn create_dir(dir: &Path) -> Result<(), Stop> {
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(|e| Stop::Fail(format!("建不了文件夹 {}：{e}", dir.display())))
}

/// 清单的指纹：路径、类型、大小、修改时间都相同，才能接着上次的进度收。
fn fingerprint(entries: &[FileEntry]) -> String {
    let mut hash = Context::new(&SHA256);
    for entry in entries {
        hash.update(entry.path.as_bytes());
        hash.update(&[0, u8::from(entry.kind == EntryKind::Dir)]);
        hash.update(&entry.size.to_le_bytes());
        hash.update(&entry.modified.to_le_bytes());
    }
    hash.finish()
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// 上次收到第几个条目为止。没有进度、或者清单变了时为 `None`。
async fn read_state(work: &Path, fingerprint: &str) -> Option<usize> {
    let text = tokio::fs::read_to_string(work.join("state")).await.ok()?;
    let mut parts = text.split_whitespace();
    if parts.next()? != STATE_TAG || parts.next()? != fingerprint {
        return None;
    }
    parts.next()?.parse().ok()
}

async fn write_state(work: &Path, fingerprint: &str, completed: usize) -> Result<(), Stop> {
    let text = format!("{STATE_TAG} {fingerprint} {completed}\n");
    tokio::fs::write(work.join("state"), text)
        .await
        .map_err(|e| Stop::Fail(format!("记不下接收进度：{e}")))
}
