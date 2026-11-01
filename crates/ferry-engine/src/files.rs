//! 文件传输：把若干文件和文件夹交给对方。设计见 ADR 0005，线上格式见 `docs/protocol.md` 的“文件”一节。
//!
//! [`Files`] 是一台设备上所有传输的控制中心，服务端和客户端各一个，跨会话保留，
//! 断线重连后据此续传。控制消息（通告、接受、取消）走数据连接，
//! 内容走每次单独打开的文件连接，文件连接始终由客户端打开。

mod manifest;
mod receive;
mod send;

use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use ferry_net::{ClientStream, Framed};
use ferry_proto::{CancelReason, FileCancel, FileOffer, Message};
use ferry_store::Store;
use ferry_types::TransferId;
use ring::rand::{SecureRandom, SystemRandom};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;
use tokio::task::AbortHandle;
use tokio::time::timeout;
use tracing::{info, warn};

use crate::error::{Error, Result};
use crate::events::{Event, Events, TransferState, TransferUpdate};
use manifest::Plan;

/// 服务端等客户端在新的文件连接上说明用途的时间。
const OPEN_WAIT: Duration = Duration::from_secs(10);

/// 会话还在、文件连接却断了时，发送方在本会话内最多重试几次。
const MAX_RETRIES: u32 = 3;

/// 传输中报告进度的最短间隔。
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

/// 客户端打开一条已完成握手的文件连接。
pub(crate) type Opener = Arc<
    dyn Fn() -> Pin<Box<dyn Future<Output = Result<Framed<ClientStream>>> + Send>> + Send + Sync,
>;

/// 一台设备上所有文件传输的控制中心。克隆后共用同一份状态。
#[derive(Clone)]
pub struct Files {
    inner: Arc<Inner>,
}

struct Inner {
    /// 正在接收的文件放在这里，每次传输一个子目录。
    partial: PathBuf,
    /// 收完的文件放在这里。
    downloads: PathBuf,
    events: Events,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    link: Option<Link>,
    /// 每次连上数据连接加一，旧连接迟到的断开据此识别。
    generation: u64,
    jobs: HashMap<TransferId, Job>,
}

/// 当前会话的数据连接。
struct Link {
    generation: u64,
    /// 要在数据连接上发出的控制消息。
    tx: UnboundedSender<Message>,
    /// 客户端打开文件连接的方式；服务端为 `None`，等客户端连过来。
    opener: Option<Opener>,
}

struct Job {
    kind: JobKind,
    /// 正在传输时的任务。
    task: Option<AbortHandle>,
    /// 本会话内因文件连接断开而重试的次数。
    retries: u32,
}

#[derive(Clone)]
enum JobKind {
    Send(Arc<Plan>),
    Receive(FileOffer),
}

impl JobKind {
    fn update(&self, done: u64, state: TransferState) -> TransferUpdate {
        match self {
            Self::Send(plan) => TransferUpdate {
                id: plan.id,
                outgoing: true,
                name: plan.name.clone(),
                items: plan.items,
                files: plan.files,
                bytes: plan.bytes,
                done,
                state,
            },
            Self::Receive(offer) => TransferUpdate {
                id: offer.id,
                outgoing: false,
                name: offer.name.clone(),
                items: offer.items,
                files: offer.files,
                bytes: offer.bytes,
                done,
                state,
            },
        }
    }
}

/// 一次传输为什么停下。
pub(crate) enum Stop {
    /// 连接断了：发送方保留任务，接收方保留已收到的部分，之后接着传。
    Net(String),
    /// 出错了，不再继续。
    Fail(String),
}

impl From<ferry_net::Error> for Stop {
    fn from(e: ferry_net::Error) -> Self {
        Self::Net(e.to_string())
    }
}

impl From<Error> for Stop {
    fn from(e: Error) -> Self {
        Self::Net(e.to_string())
    }
}

impl Files {
    /// 收到的文件放在 `store` 的下载目录，接收中的部分放在它的 `partial/`。
    pub fn new(store: &Store, events: Events) -> Self {
        Self {
            inner: Arc::new(Inner {
                partial: store.partial_dir(),
                downloads: store.downloads_dir(),
                events,
                state: Mutex::new(State::default()),
            }),
        }
    }

    /// 把这些文件和文件夹发给当前连着的对方，返回这次传输的编号。
    ///
    /// 展开文件夹需要读磁盘，在后台线程完成。对方会自动接受，进展经 [`Event::Transfer`] 报告。
    pub async fn send(&self, paths: Vec<PathBuf>) -> Result<TransferId> {
        if self.lock().link.is_none() {
            return Err(Error::NotConnected);
        }
        let id = new_id()?;
        let plan = tokio::task::spawn_blocking(move || manifest::build(id, &paths))
            .await
            .map_err(|_| Error::Transfer("整理要发送的文件时出错".into()))?
            .map_err(Error::Transfer)?;
        let plan = Arc::new(plan);
        let offer = plan.offer();
        {
            let mut state = self.lock();
            let Some(link) = &state.link else {
                return Err(Error::NotConnected);
            };
            let _ = link.tx.send(Message::FileOffer(offer));
            state.jobs.insert(
                id,
                Job {
                    kind: JobKind::Send(plan.clone()),
                    task: None,
                    retries: 0,
                },
            );
        }
        info!(
            name = %plan.name,
            files = plan.files,
            bytes = plan.bytes,
            "准备发送文件"
        );
        self.emit(JobKind::Send(plan).update(0, TransferState::Waiting));
        Ok(id)
    }

    /// 取消一次传输。对方会收到通知；接收方删掉已收到的部分。
    pub fn cancel(&self, id: TransferId) {
        self.stop(id, false);
    }

    /// 连上一条数据连接。之前没传完的发送任务立即重新通告，接着传。
    ///
    /// 返回的 [`FileLink`] 交给数据连接的任务：它要发出 `outbound` 中的消息，
    /// 并把收到的文件控制消息交给 [`Files::on_control`]。丢弃它即表示数据连接已断开。
    pub(crate) fn attach(&self, opener: Option<Opener>) -> FileLink {
        let (tx, outbound) = mpsc::unbounded_channel();
        let (generation, resumed) = {
            let mut state = self.lock();
            state.generation += 1;
            let generation = state.generation;
            state.link = Some(Link {
                generation,
                tx: tx.clone(),
                opener,
            });
            let resumed: Vec<Arc<Plan>> = state
                .jobs
                .values_mut()
                .filter_map(|job| match &job.kind {
                    JobKind::Send(plan) => {
                        job.retries = 0;
                        Some(plan.clone())
                    }
                    JobKind::Receive(_) => None,
                })
                .collect();
            (generation, resumed)
        };
        for plan in resumed {
            info!(name = %plan.name, "重新连上，接着发送");
            let _ = tx.send(Message::FileOffer(plan.offer()));
            self.emit(JobKind::Send(plan).update(0, TransferState::Waiting));
        }
        FileLink {
            files: self.clone(),
            outbound,
            generation,
        }
    }

    /// 数据连接上收到的文件控制消息。
    pub(crate) fn on_control(&self, message: Message) {
        match message {
            Message::FileOffer(offer) => self.on_offer(offer),
            Message::FileAccept { id } => self.on_accept(id),
            Message::FileCancel(FileCancel { id, .. }) => self.stop(id, true),
            _ => {}
        }
    }

    /// 服务端收到一条文件连接（握手已完成）：等它说明是哪次传输，再开始收发。
    pub(crate) fn serve<S>(&self, mut framed: Framed<S>)
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let files = self.clone();
        tokio::spawn(async move {
            let id = match timeout(OPEN_WAIT, framed.recv()).await {
                Ok(Ok(Some(Message::FileOpen { id }))) => id,
                _ => {
                    warn!("新的文件连接没有说明是哪次传输，已关闭");
                    return;
                }
            };
            let idle = files.lock().jobs.get(&id).is_some_and(|j| j.task.is_none());
            if !idle {
                warn!(%id, "文件连接对应的传输不存在或已在进行，已关闭");
                return;
            }
            let worker = files.clone();
            files.launch(id, async move { worker.run(id, framed).await });
        });
    }

    fn on_offer(&self, offer: FileOffer) {
        let id = offer.id;
        let (tx, opener) = {
            let mut state = self.lock();
            if state.jobs.contains_key(&id) {
                return;
            }
            let Some(link) = &state.link else {
                return;
            };
            let found = (link.tx.clone(), link.opener.clone());
            state.jobs.insert(
                id,
                Job {
                    kind: JobKind::Receive(offer.clone()),
                    task: None,
                    retries: 0,
                },
            );
            found
        };
        info!(name = %offer.name, files = offer.files, bytes = offer.bytes, "对方发来文件，开始接收");
        self.emit(JobKind::Receive(offer).update(0, TransferState::Waiting));
        let _ = tx.send(Message::FileAccept { id });
        if let Some(opener) = opener {
            self.open_and_run(id, opener);
        }
    }

    fn on_accept(&self, id: TransferId) {
        let opener = {
            let state = self.lock();
            let sending = state
                .jobs
                .get(&id)
                .is_some_and(|j| j.task.is_none() && matches!(j.kind, JobKind::Send(_)));
            match &state.link {
                Some(link) if sending => link.opener.clone(),
                _ => None,
            }
        };
        // 服务端没有 opener：等客户端打开文件连接。
        if let Some(opener) = opener {
            self.open_and_run(id, opener);
        }
    }

    /// 客户端：打开文件连接并开始收发。
    fn open_and_run(&self, id: TransferId, opener: Opener) {
        let files = self.clone();
        self.launch(id, async move {
            let mut framed = opener().await?;
            framed.send(&Message::FileOpen { id }).await?;
            files.run(id, framed).await
        });
    }

    async fn run<S>(&self, id: TransferId, mut framed: Framed<S>) -> Result<Vec<PathBuf>, Stop>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let kind = self.lock().jobs.get(&id).map(|job| job.kind.clone());
        let Some(kind) = kind else {
            return Err(Stop::Fail("这次传输已经不存在".into()));
        };
        let mut progress = Progress::new(self.inner.events.clone(), kind.clone());
        match &kind {
            JobKind::Send(plan) => send::run(&mut framed, plan, &mut progress).await,
            JobKind::Receive(offer) => {
                receive::run(
                    &mut framed,
                    offer,
                    &self.inner.partial,
                    &self.inner.downloads,
                    &mut progress,
                )
                .await
            }
        }
    }

    /// 在后台运行一次传输，结束后交给 [`Files::finish`]。
    ///
    /// 任务先记下自己的句柄再开工，取消时一定能找到它。
    fn launch<F>(&self, id: TransferId, work: F)
    where
        F: Future<Output = Result<Vec<PathBuf>, Stop>> + Send + 'static,
    {
        let (go, ready) = oneshot::channel::<()>();
        let files = self.clone();
        let handle = tokio::spawn(async move {
            if ready.await.is_err() {
                return;
            }
            let outcome = work.await;
            files.finish(id, outcome);
        });
        {
            let mut state = self.lock();
            match state.jobs.get_mut(&id) {
                Some(job) if job.task.is_none() => job.task = Some(handle.abort_handle()),
                _ => {
                    handle.abort();
                    return;
                }
            }
        }
        let _ = go.send(());
    }

    fn finish(&self, id: TransferId, outcome: Result<Vec<PathBuf>, Stop>) {
        match outcome {
            Ok(saved) => {
                let Some(job) = self.lock().jobs.remove(&id) else {
                    return;
                };
                let total = match &job.kind {
                    JobKind::Send(plan) => plan.bytes,
                    JobKind::Receive(offer) => offer.bytes,
                };
                match saved.first() {
                    Some(first) => info!(at = %first.display(), "文件收好了"),
                    None => info!("文件发送完成"),
                }
                self.emit(job.kind.update(total, TransferState::Done { saved }));
            }
            Err(Stop::Net(reason)) => {
                let (kind, retry) = {
                    let mut state = self.lock();
                    let tx = state.link.as_ref().map(|link| link.tx.clone());
                    let Some(job) = state.jobs.get_mut(&id) else {
                        return;
                    };
                    job.task = None;
                    let kind = job.kind.clone();
                    let retry = match (&kind, tx) {
                        (JobKind::Send(plan), Some(tx)) if job.retries < MAX_RETRIES => {
                            job.retries += 1;
                            Some((tx, plan.offer()))
                        }
                        _ => None,
                    };
                    if matches!(kind, JobKind::Receive(_)) {
                        // 发送方会用同一个编号重新通告，届时再接着收。
                        state.jobs.remove(&id);
                    }
                    (kind, retry)
                };
                warn!(%reason, "文件传输中断");
                self.emit(kind.update(0, TransferState::Interrupted));
                if let Some((tx, offer)) = retry {
                    let _ = tx.send(Message::FileOffer(offer));
                }
            }
            Err(Stop::Fail(reason)) => {
                let (job, tx) = {
                    let mut state = self.lock();
                    (
                        state.jobs.remove(&id),
                        state.link.as_ref().map(|link| link.tx.clone()),
                    )
                };
                let Some(job) = job else {
                    return;
                };
                warn!(%reason, "文件传输失败");
                if let Some(tx) = tx {
                    let cancel = FileCancel {
                        id,
                        reason: CancelReason::UNAVAILABLE,
                    };
                    let _ = tx.send(Message::FileCancel(cancel));
                }
                if matches!(job.kind, JobKind::Receive(_)) {
                    self.discard_partial(id);
                }
                self.emit(job.kind.update(0, TransferState::Failed { reason }));
            }
        }
    }

    /// 停止并忘掉一次传输。`by_peer` 表示是对方取消的，不必再通知对方。
    fn stop(&self, id: TransferId, by_peer: bool) {
        let (job, tx) = {
            let mut state = self.lock();
            (
                state.jobs.remove(&id),
                state.link.as_ref().map(|link| link.tx.clone()),
            )
        };
        let Some(job) = job else {
            return;
        };
        if let Some(task) = &job.task {
            task.abort();
        }
        if !by_peer && let Some(tx) = tx {
            let cancel = FileCancel {
                id,
                reason: CancelReason::CANCELLED,
            };
            let _ = tx.send(Message::FileCancel(cancel));
        }
        if matches!(job.kind, JobKind::Receive(_)) {
            self.discard_partial(id);
        }
        info!(%id, by_peer, "文件传输已取消");
        self.emit(job.kind.update(0, TransferState::Cancelled { by_peer }));
    }

    /// 数据连接断开：停下正在进行的传输。发送任务留着，下次连上接着传。
    fn detach(&self, generation: u64) {
        let stopped: Vec<JobKind> = {
            let mut state = self.lock();
            if state
                .link
                .as_ref()
                .is_none_or(|l| l.generation != generation)
            {
                return;
            }
            state.link = None;
            let stopped = state
                .jobs
                .values_mut()
                .map(|job| {
                    if let Some(task) = job.task.take() {
                        task.abort();
                    }
                    job.kind.clone()
                })
                .collect();
            state
                .jobs
                .retain(|_, job| matches!(job.kind, JobKind::Send(_)));
            stopped
        };
        for kind in stopped {
            self.emit(kind.update(0, TransferState::Interrupted));
        }
    }

    /// 删掉一次接收已经收到的部分。
    fn discard_partial(&self, id: TransferId) {
        let dir = self.inner.partial.join(id.to_string());
        tokio::task::spawn_blocking(move || {
            let _ = std::fs::remove_dir_all(dir);
        });
    }

    fn emit(&self, update: TransferUpdate) {
        self.inner.events.emit(Event::Transfer(update));
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.inner.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// 一条数据连接与文件传输之间的接口。丢弃即表示数据连接已断开。
pub(crate) struct FileLink {
    files: Files,
    outbound: UnboundedReceiver<Message>,
    generation: u64,
}

impl FileLink {
    /// 下一条要在数据连接上发出的控制消息。
    pub(crate) async fn next(&mut self) -> Option<Message> {
        self.outbound.recv().await
    }

    pub(crate) fn on_control(&self, message: Message) {
        self.files.on_control(message);
    }
}

impl Drop for FileLink {
    fn drop(&mut self) {
        self.files.detach(self.generation);
    }
}

/// 传输进度，限频报告。
pub(crate) struct Progress {
    events: Events,
    kind: JobKind,
    done: u64,
    last: Instant,
}

impl Progress {
    fn new(events: Events, kind: JobKind) -> Self {
        Self {
            events,
            kind,
            done: 0,
            last: Instant::now(),
        }
    }

    /// 开始传内容。`done` 是续传前已经传过的字节数。
    pub(crate) fn start(&mut self, done: u64) {
        self.done = done;
        self.report();
    }

    pub(crate) fn add(&mut self, bytes: u64) {
        self.done += bytes;
        if self.last.elapsed() >= PROGRESS_INTERVAL {
            self.report();
        }
    }

    fn report(&mut self) {
        self.last = Instant::now();
        let update = self.kind.update(self.done, TransferState::Running);
        self.events.emit(Event::Transfer(update));
    }
}

fn new_id() -> Result<TransferId> {
    let mut id = [0u8; 16];
    SystemRandom::new()
        .fill(&mut id)
        .map_err(|_| Error::Transfer("系统随机数不可用".into()))?;
    Ok(TransferId(id))
}
