//! 剪贴板同步：在数据连接上运行，服务端与客户端共用。
//!
//! 同一时刻只有一台是“活动端”（光标所在的那台）。规则只有两条：
//!
//! - **通告**：活动端轮询本机剪贴板，变了就发 `ClipboardOffer`，只说有哪些格式、多大；
//!   成为非活动端的那一刻再检查一次，刚复制完就切走也不会漏。
//! - **拉取**：手上有对方未拉取的通告、且本机是活动端时，立即拉取并写入本机剪贴板。
//!
//! 所以复制多次只传最后一次；光标不过去，对方的剪贴板就不会被覆盖。
//! 例外是刚连上时：服务端先通告一次当前内容，见 [`Start`]。
//! 写入后记下剪贴板的变化计数，自己写入的内容不会再被当作新复制通告回去。

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use ferry_net::Framed;
use ferry_platform::Clipboard;
use ferry_proto::limits::MAX_CHUNK_LEN;
use ferry_proto::{
    Cancel, CancelReason, Chunk, ClipboardItem, ClipboardOffer, ClipboardRequest, Message,
};
use ferry_types::{ClipboardContent, ClipboardFormat};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::{MissedTickBehavior, interval};
use tracing::{debug, info, warn};

use crate::error::{Error, Result};
use crate::link::send;

/// 活动端检查本机剪贴板的间隔。
const POLL_INTERVAL: Duration = Duration::from_millis(250);

/// 单个格式的大小上限，超过的不通告也不拉取。
pub(crate) const MAX_ITEM_SIZE: u64 = 32 * 1024 * 1024;

/// 服务端与客户端各持有一份，会话之间复用。
pub(crate) type SharedClipboard<C> = Arc<Mutex<C>>;

/// 正在运行的同步任务，丢弃即停止。
pub(crate) struct ClipboardTask(JoinHandle<()>);

impl Drop for ClipboardTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// 连上时谁先通告本机剪贴板。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Start {
    /// 会话开始时光标总在服务端，它的剪贴板就是用户眼下的剪贴板，连上就通告。
    Announce,
    /// 客户端的旧内容只记下计数，免得一连上就覆盖服务端。
    Quiet,
}

/// 在数据连接上启动剪贴板同步。`active` 表示光标是否在本机。
///
/// 数据连接比输入连接晚建立，期间光标可能已经切走，
/// 所以连上时是否通告由 `start` 决定，而不看这一刻谁是活动端。
pub(crate) fn spawn<S, C>(
    framed: Framed<S>,
    clipboard: SharedClipboard<C>,
    active: watch::Receiver<bool>,
    start: Start,
) -> ClipboardTask
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    C: Clipboard,
{
    ClipboardTask(tokio::spawn(async move {
        let mut sync = ClipboardSync::new(framed, clipboard, active);
        match sync.run(start).await {
            Ok(()) => debug!("数据连接已关闭"),
            Err(e) => warn!("剪贴板同步中断：{e}"),
        }
    }))
}

struct ClipboardSync<S, C> {
    framed: Framed<S>,
    clipboard: SharedClipboard<C>,
    active: watch::Receiver<bool>,
    /// 最近一次看到的本机剪贴板变化计数。
    seen: u64,
    /// 最近一次通告出去的本机内容，等对方来拉取。
    offered: Option<(u32, ClipboardContent)>,
    next_seq: u32,
    /// 对方最近一次通告、还没拉取的内容。
    offer: Option<ClipboardOffer>,
    fetch: Option<Fetch>,
    next_transfer: u32,
}

/// 正在拉取的一份对方内容。各格式都到齐后一次写入本机剪贴板。
struct Fetch {
    seq: u32,
    /// 传输号 → 格式、通告的大小、已收到的数据。
    pending: HashMap<u32, (ClipboardFormat, u64, Vec<u8>)>,
    done: ClipboardContent,
}

impl<S, C> ClipboardSync<S, C>
where
    S: AsyncRead + AsyncWrite + Unpin,
    C: Clipboard,
{
    fn new(
        framed: Framed<S>,
        clipboard: SharedClipboard<C>,
        active: watch::Receiver<bool>,
    ) -> Self {
        Self {
            framed,
            clipboard,
            active,
            seen: 0,
            offered: None,
            next_seq: 1,
            offer: None,
            fetch: None,
            next_transfer: 1,
        }
    }

    async fn run(&mut self, start: Start) -> Result<()> {
        let count = self.lock().change_count();
        self.seen = count;
        if start == Start::Announce {
            self.offer_local().await?;
        }

        let mut tick = interval(POLL_INTERVAL);
        tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                message = self.framed.recv() => match message? {
                    None | Some(Message::Bye(_)) => return Ok(()),
                    Some(message) => self.on_message(message).await?,
                },
                changed = self.active.changed() => {
                    if changed.is_err() {
                        return Ok(()); // 会话已结束
                    }
                    if self.is_active() {
                        self.start_fetch().await?;
                    } else {
                        self.poll().await?;
                    }
                }
                _ = tick.tick() => {
                    if self.is_active() {
                        self.poll().await?;
                    }
                }
            }
        }
    }

    async fn on_message(&mut self, message: Message) -> Result<()> {
        match message {
            Message::Ping { nonce } => send(&mut self.framed, &Message::Pong { nonce }).await?,
            Message::ClipboardOffer(offer) => {
                debug!(seq = offer.seq, items = ?offer.items, "对方剪贴板有新内容");
                // 新通告让正在拉取的旧内容作废；迟到的旧分块按未知传输号丢弃。
                self.fetch = None;
                self.offer = Some(offer);
                if self.is_active() {
                    self.start_fetch().await?;
                }
            }
            Message::ClipboardRequest(request) => self.serve(request).await?,
            Message::Chunk(chunk) => self.on_chunk(chunk)?,
            Message::Cancel(cancel) => {
                if let Some(fetch) = self.fetch.as_mut()
                    && fetch.pending.remove(&cancel.transfer).is_some()
                {
                    debug!(reason = cancel.reason.0, "对方取消了一个格式的传输");
                    self.finish_if_done()?;
                }
            }
            other => debug!(?other, "数据连接上忽略消息"),
        }
        Ok(())
    }

    /// 本机剪贴板变了就通告。
    async fn poll(&mut self) -> Result<()> {
        if self.lock().change_count() != self.seen {
            self.offer_local().await?;
        }
        Ok(())
    }

    async fn offer_local(&mut self) -> Result<()> {
        let (count, content) = {
            let mut clipboard = self.lock();
            (clipboard.change_count(), clipboard.read())
        };
        self.seen = count;
        let content = content.unwrap_or_else(|e| {
            warn!("读取本机剪贴板失败：{e}");
            ClipboardContent::new()
        });

        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        // 没有可共享的格式也照样通告（空列表），让对方丢掉之前的旧通告。
        let items = content
            .iter()
            .map(|(format, data)| ClipboardItem {
                format,
                size: data.len() as u64,
            })
            .filter(|item| item.size <= MAX_ITEM_SIZE)
            .collect();
        let offer = ClipboardOffer { seq, items };
        debug!(seq, items = ?offer.items, "通告本机剪贴板");
        self.offered = Some((seq, content));
        send(&mut self.framed, &Message::ClipboardOffer(offer)).await
    }

    /// 拉取对方通告中本机支持的格式。
    async fn start_fetch(&mut self) -> Result<()> {
        if self.fetch.is_some() {
            return Ok(());
        }
        let Some(offer) = self.offer.take() else {
            return Ok(());
        };
        let supported = self.lock().formats();
        let mut fetch = Fetch {
            seq: offer.seq,
            pending: HashMap::new(),
            done: ClipboardContent::new(),
        };
        for item in offer.items {
            if !supported.contains(&item.format) || item.size > MAX_ITEM_SIZE {
                continue;
            }
            let transfer = self.next_transfer;
            self.next_transfer = self.next_transfer.wrapping_add(1);
            let request = ClipboardRequest {
                seq: offer.seq,
                format: item.format,
                transfer,
            };
            send(&mut self.framed, &Message::ClipboardRequest(request)).await?;
            fetch
                .pending
                .insert(transfer, (item.format, item.size, Vec::new()));
        }
        if !fetch.pending.is_empty() {
            self.fetch = Some(fetch);
        }
        Ok(())
    }

    /// 回应对方的拉取：按块发出内容。过期的请求回 `Cancel(STALE)`。
    async fn serve(&mut self, request: ClipboardRequest) -> Result<()> {
        let data = match &self.offered {
            Some((seq, content)) if *seq == request.seq => content.get(request.format),
            _ => None,
        };
        let Some(data) = data else {
            let cancel = Cancel {
                transfer: request.transfer,
                reason: CancelReason::STALE,
            };
            return send(&mut self.framed, &Message::Cancel(cancel)).await;
        };

        // 空内容也要发一块 last = true，对方才知道传完了。
        let count = data.len().div_ceil(MAX_CHUNK_LEN).max(1);
        for (i, part) in data
            .chunks(MAX_CHUNK_LEN)
            .chain(data.is_empty().then_some(&[][..]))
            .enumerate()
        {
            let chunk = Chunk {
                transfer: request.transfer,
                last: i + 1 == count,
                data: part.to_vec(),
            };
            send(&mut self.framed, &Message::Chunk(chunk)).await?;
        }
        Ok(())
    }

    fn on_chunk(&mut self, chunk: Chunk) -> Result<()> {
        let Some(fetch) = self.fetch.as_mut() else {
            return Ok(());
        };
        let Some((_, size, data)) = fetch.pending.get_mut(&chunk.transfer) else {
            return Ok(()); // 已作废的传输
        };
        data.extend_from_slice(&chunk.data);
        if data.len() as u64 > *size {
            return Err(Error::Protocol("剪贴板内容超过了通告的大小"));
        }
        if chunk.last {
            let (format, _, data) = fetch.pending.remove(&chunk.transfer).expect("刚刚查到过");
            fetch.done.insert(format, data);
            self.finish_if_done()?;
        }
        Ok(())
    }

    /// 所有格式都到齐（或被取消）后写入本机剪贴板。
    fn finish_if_done(&mut self) -> Result<()> {
        if self.fetch.as_ref().is_some_and(|f| !f.pending.is_empty()) {
            return Ok(());
        }
        let Some(fetch) = self.fetch.take() else {
            return Ok(());
        };
        if fetch.done.is_empty() {
            return Ok(());
        }
        let written = self.lock().write(&fetch.done);
        match written {
            Ok(count) => {
                self.seen = count;
                info!(
                    seq = fetch.seq,
                    bytes = fetch.done.total_len(),
                    "已同步对方的剪贴板"
                );
            }
            Err(e) => warn!("写入本机剪贴板失败：{e}"),
        }
        Ok(())
    }

    fn is_active(&self) -> bool {
        *self.active.borrow()
    }

    fn lock(&self) -> MutexGuard<'_, C> {
        self.clipboard.lock().unwrap_or_else(|e| e.into_inner())
    }
}
