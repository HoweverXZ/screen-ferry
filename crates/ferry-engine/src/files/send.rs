//! 发送方在文件连接上的流程：清单 → 等续传点 → 按顺序发内容 → 等对方收好。

use std::path::{Path, PathBuf};
use std::time::Duration;

use ferry_net::Framed;
use ferry_proto::limits::{MAX_CHUNK_LEN, MAX_FILE_LIST};
use ferry_proto::{EntryKind, FileEnd, FileEntry, FileList, FileResume, Message};
use ring::digest::{Context, SHA256};
use tokio::fs::File;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};

use super::manifest::Plan;
use super::{Progress, Stop};

/// 等接收方说明从哪里开始。它可能要先把已收到的大文件读一遍算校验和。
const RESUME_WAIT: Duration = Duration::from_secs(300);

/// 发完后等接收方把文件放好。跨磁盘时要复制，可能比较久。
const DONE_WAIT: Duration = Duration::from_secs(3600);

pub(super) async fn run<S>(
    framed: &mut Framed<S>,
    plan: &Plan,
    progress: &mut Progress,
) -> Result<Vec<PathBuf>, Stop>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let count = plan.entries.chunks(MAX_FILE_LIST).count();
    for (i, entries) in plan.entries.chunks(MAX_FILE_LIST).enumerate() {
        let list = FileList {
            entries: entries.to_vec(),
            last: i + 1 == count,
        };
        framed.send(&Message::FileList(list)).await?;
    }

    let FileResume { index, offset } = match recv(framed, RESUME_WAIT).await? {
        Message::FileResume(resume) => resume,
        _ => return Err(Stop::Fail("对方没有说明从哪里开始".into())),
    };
    let start = usize::try_from(index).unwrap_or(usize::MAX);
    let valid = match plan.entries.get(start) {
        Some(entry) if entry.kind == EntryKind::File => offset <= entry.size,
        Some(_) => offset == 0,
        None => start == plan.entries.len() && offset == 0,
    };
    if !valid {
        return Err(Stop::Fail("对方给的续传位置不对".into()));
    }
    let before: u64 = plan.entries[..start]
        .iter()
        .filter(|e| e.kind == EntryKind::File)
        .map(|e| e.size)
        .sum();
    progress.start(before + offset);

    let mut buf = vec![0u8; MAX_CHUNK_LEN];
    for i in start..plan.entries.len() {
        let entry = &plan.entries[i];
        if entry.kind == EntryKind::Dir {
            continue;
        }
        let from = if i == start { offset } else { 0 };
        let sha256 = send_file(framed, &plan.sources[i], entry, from, &mut buf, progress).await?;
        let index = u32::try_from(i).expect("条目数有上限");
        framed
            .send(&Message::FileEnd(FileEnd { index, sha256 }))
            .await?;
    }

    match recv(framed, DONE_WAIT).await? {
        Message::FileDone => Ok(Vec::new()),
        _ => Err(Stop::Fail("对方没有确认收好".into())),
    }
}

/// 发出一个文件从 `from` 开始的内容，返回整个文件的 SHA-256。
async fn send_file<S>(
    framed: &mut Framed<S>,
    path: &Path,
    entry: &FileEntry,
    from: u64,
    buf: &mut [u8],
    progress: &mut Progress,
) -> Result<[u8; 32], Stop>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let fail = |what: &str, e: std::io::Error| Stop::Fail(format!("{what} {}：{e}", entry.path));
    let mut file = File::open(path).await.map_err(|e| fail("打不开", e))?;
    let len = file.metadata().await.map_err(|e| fail("读不了", e))?.len();
    if len != entry.size {
        return Err(Stop::Fail(format!("{} 在发送期间改变了", entry.path)));
    }
    let mut hash = Context::new(&SHA256);
    let mut sent = 0;
    // 续传：已经发过的部分只读出来算校验和。
    while sent < from {
        let n = read_some(&mut file, buf, from - sent, entry).await?;
        hash.update(&buf[..n]);
        sent += n as u64;
    }
    while sent < entry.size {
        let n = read_some(&mut file, buf, entry.size - sent, entry).await?;
        hash.update(&buf[..n]);
        framed
            .send(&Message::FileData {
                data: buf[..n].to_vec(),
            })
            .await?;
        sent += n as u64;
        progress.add(n as u64);
    }
    Ok(hash
        .finish()
        .as_ref()
        .try_into()
        .expect("SHA-256 是 32 字节"))
}

/// 最多读 `remaining` 个字节。文件比清单短了就是出错。
async fn read_some(
    file: &mut File,
    buf: &mut [u8],
    remaining: u64,
    entry: &FileEntry,
) -> Result<usize, Stop> {
    let want = usize::try_from(remaining).map_or(buf.len(), |r| r.min(buf.len()));
    let n = file
        .read(&mut buf[..want])
        .await
        .map_err(|e| Stop::Fail(format!("读不了 {}：{e}", entry.path)))?;
    if n == 0 {
        return Err(Stop::Fail(format!("{} 在发送期间变短了", entry.path)));
    }
    Ok(n)
}

/// 收一条消息，超时、断开或对方告别都算连接断了。
pub(super) async fn recv<S>(framed: &mut Framed<S>, wait: Duration) -> Result<Message, Stop>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    match tokio::time::timeout(wait, framed.recv()).await {
        Err(_) => Err(Stop::Net("对方长时间没有响应".into())),
        Ok(Err(e)) => Err(e.into()),
        Ok(Ok(None | Some(Message::Bye(_)))) => Err(Stop::Net("文件连接已关闭".into())),
        Ok(Ok(Some(message))) => Ok(message),
    }
}
