//! 边缘投递：拖着文件靠近朝向对方的屏幕边缘时，贴边滑出胶囊，放上去就发送；
//! 收发过程中胶囊显示进度，发送方完成后自动收起，接收方收完变成卡片。见 ADR 0005。
//!
//! 引擎经 [`EdgeFeed`] 报告光标和朝向对方的那条边；文件传输的进展由常驻后台转来。
//! 这里决定显示哪种样子，[`Capsule`] 算出动画中的每一帧，交给平台的 [`Overlay`] 画出来。
//!
//! 两端联动：本机拖着文件靠近边缘时，经引擎把提示（`DropHint`）发给对方；对方在相邻边上
//! 同一位置提前滑出接收胶囊，放下后接收进度也出现在那里。

use std::path::PathBuf;
use std::time::Duration;

use ferry_core::Side;
use ferry_core::capsule::{Action, Avatar, Capsule, Glyph, Look, Tile, hit, place};
use ferry_platform::{Overlay, OverlayEvent};
use ferry_proto::{DropHint, HintState};
use ferry_types::{Frame, Rgba, TransferId};
use tokio::sync::{mpsc, watch};
use tokio::time::{Instant, sleep_until};
use tracing::warn;

use crate::events::{TransferState, TransferUpdate};
use crate::files::Files;

/// 离边缘多近（点）开始出现胶囊。
const NEAR: f32 = 210.0;
/// 离边缘多近（点）胶囊长到最大。
const CLOSE: f32 = 80.0;
/// 留意拖动时贴边接收条的宽度（点）。
const STRIP: f32 = 6.0;
/// 动画的帧间隔。
const FRAME: Duration = Duration::from_millis(16);
/// “已发送”“已取消”这类提示停留多久。
const BRIEF: Duration = Duration::from_millis(1800);
/// 出错提示停留多久。
const NOTICE: Duration = Duration::from_secs(4);
/// 收到后的卡片停留多久。
const CARD: Duration = Duration::from_secs(15);
/// 对方说“离开了”之后，接收胶囊再停留多久。
const GRACE: Duration = Duration::from_millis(300);
/// 对方直接从胶囊上“离开”，多半是放下了文件：提示和传输走不同的连接，先到的是“离开”，
/// 多留一会儿等传输开始，胶囊就不会收起又弹出来。
const DROP_GRACE: Duration = Duration::from_millis(2000);
/// 两条拖动提示之间至少隔多久；状态变化（靠近、到胶囊上、离开）不受限制。
const HINT_GAP: Duration = Duration::from_millis(50);
/// 提示里沿边位置的满量程。
const ALONG_FULL: f32 = 10_000.0;

/// 本机光标的现状。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pointer {
    /// 桌面坐标。
    pub x: f32,
    pub y: f32,
    /// 是否按着鼠标键（拖着东西）。
    pub held: bool,
    /// 光标是否在本机。
    pub here: bool,
}

/// 朝向对方的那条边。
#[derive(Clone, Debug, PartialEq)]
pub struct Edge {
    /// 对方在本机的哪一侧。
    pub side: Side,
    /// 本机屏幕的范围（桌面坐标）。
    pub screen: Frame,
    /// 每个点对应多少桌面单位。
    pub scale: f32,
    pub peer: String,
    /// 是否连着对方。断开后仍保留这条边，拖文件过来时显示“未连接”。
    pub online: bool,
}

/// 引擎往边缘投递报告光标和边的地方。克隆后共用；没有边缘投递时什么也不做。
#[derive(Clone)]
pub struct EdgeFeed {
    pointer: watch::Sender<Pointer>,
    edge: watch::Sender<Option<Edge>>,
    /// 本机拖动中的提示，由引擎发给对方。`None` 表示没有在拖（对方应收起）。
    outgoing: watch::Sender<Option<DropHint>>,
    /// 对方发来的拖动提示。
    incoming: watch::Sender<Option<DropHint>>,
}

impl EdgeFeed {
    /// 不需要边缘投递（命令行模式、测试）。
    pub fn none() -> Self {
        Self {
            pointer: watch::channel(Pointer::default()).0,
            edge: watch::channel(None).0,
            outgoing: watch::channel(None).0,
            incoming: watch::channel(None).0,
        }
    }

    /// 本机要发给对方的拖动提示，变了就发。
    pub(crate) fn hints(&self) -> watch::Receiver<Option<DropHint>> {
        self.outgoing.subscribe()
    }

    /// 对方发来一条拖动提示；`None` 表示会话结束。
    pub(crate) fn remote(&self, hint: Option<DropHint>) {
        self.incoming.send_replace(hint);
    }

    /// 引擎停下了（暂停共享、重启）：忘掉那条边、对方的提示和光标，胶囊随之收起。
    pub(crate) fn reset(&self) {
        self.edge.send_replace(None);
        self.incoming.send_replace(None);
        self.pointer.send_replace(Pointer::default());
    }

    /// 光标或按键变了。只在真的变了时唤醒边缘投递。
    pub(crate) fn pointer(&self, update: impl FnOnce(&mut Pointer)) {
        self.pointer.send_if_modified(|p| {
            let before = *p;
            update(p);
            *p != before
        });
    }

    /// 朝向对方的边变了；没有连着能收文件的对方时为 `None`。
    pub(crate) fn edge(&self, edge: Option<Edge>) {
        self.edge.send_if_modified(|e| {
            let changed = *e != edge;
            *e = edge;
            changed
        });
    }
}

/// 正在运行的边缘投递。丢弃即停止，胶囊随之收起。
pub struct EdgeDrop {
    feed: EdgeFeed,
    transfers: mpsc::UnboundedSender<TransferUpdate>,
    accent: watch::Sender<Rgba>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for EdgeDrop {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl EdgeDrop {
    /// 在后台启动边缘投递。放下的文件交给 `files` 发送。
    pub fn spawn<O: Overlay + 'static>(mut overlay: O, files: Files, accent: Rgba) -> Self {
        let (pointer, pointer_rx) = watch::channel(Pointer::default());
        let (edge, edge_rx) = watch::channel(None);
        let (outgoing, _) = watch::channel(None);
        let (incoming, incoming_rx) = watch::channel(None);
        let (transfers, transfers_rx) = mpsc::unbounded_channel();
        let (accent_tx, accent_rx) = watch::channel(accent);
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let started = overlay.start(Box::new(move |event| {
            let _ = events_tx.send(event);
        }));
        if let Err(e) = started {
            warn!("屏幕边缘的浮层用不了，拖文件到边缘不会有反应：{e}");
        }
        let worker = Worker {
            overlay,
            files,
            pointer: pointer_rx,
            edge: edge_rx,
            outgoing: outgoing.clone(),
            incoming: incoming_rx,
            transfers: transfers_rx,
            events: events_rx,
            accent: accent_rx,
            capsule: Capsule::new(),
            drag: None,
            job: None,
            brief: None,
            card: None,
            target: None,
            pending_along: None,
            watching: None,
            remote: None,
            sent: None,
            sent_at: Instant::now(),
            hint_due: None,
            shown: false,
            moving: false,
            dirty: false,
        };
        Self {
            feed: EdgeFeed {
                pointer,
                edge,
                outgoing,
                incoming,
            },
            transfers,
            accent: accent_tx,
            task: tokio::spawn(worker.run()),
        }
    }

    /// 交给引擎的报告入口。
    pub fn feed(&self) -> EdgeFeed {
        self.feed.clone()
    }

    /// 文件传输有了进展。
    pub fn transfer(&self, update: TransferUpdate) {
        let _ = self.transfers.send(update);
    }

    /// 强调色变了。
    pub fn set_accent(&self, accent: Rgba) {
        self.accent.send_replace(accent);
    }
}

/// 一次正在进行的拖动。
struct Drag {
    /// 按下鼠标那一刻拖放剪贴板的变化计数；之后变了，说明开始拖新的东西。
    count: i64,
    /// 拖的文件：还没判断时为 `None`，判断出不是文件时为空。
    files: Option<Vec<PathBuf>>,
    /// 拖着的东西在胶囊上。
    over: bool,
    /// 拖的是什么：第一项的名字、几项、总大小（有文件夹时不算，记为 0）。
    summary: Option<(String, u32, u64)>,
}

/// 对方正在拖过来的东西。
struct Remote {
    hint: DropHint,
    /// 对方说“离开了”之后，接收胶囊留到什么时候。
    until: Option<Instant>,
}

/// 正在显示进度的传输。
struct Job {
    update: TransferUpdate,
    /// 沿边的位置：发送时是放下的位置，接收时在屏幕中间。
    along: f32,
}

/// 收到后的卡片。
struct Card {
    saved: Vec<PathBuf>,
    look: Look,
    along: f32,
    until: Instant,
}

struct Worker<O> {
    overlay: O,
    files: Files,
    pointer: watch::Receiver<Pointer>,
    edge: watch::Receiver<Option<Edge>>,
    outgoing: watch::Sender<Option<DropHint>>,
    incoming: watch::Receiver<Option<DropHint>>,
    transfers: mpsc::UnboundedReceiver<TransferUpdate>,
    events: mpsc::UnboundedReceiver<OverlayEvent>,
    accent: watch::Receiver<Rgba>,
    capsule: Capsule,
    drag: Option<Drag>,
    job: Option<Job>,
    /// 一闪而过的提示：样子、位置、到什么时候。
    brief: Option<(Look, f32, Instant)>,
    card: Option<Card>,
    /// 上一次要求胶囊显示成的样子和位置。
    target: Option<(Look, Frame)>,
    /// 刚放下文件、进度还没来时，胶囊的位置。
    pending_along: Option<f32>,
    /// 正在请平台留意的贴边区域。
    watching: Option<Frame>,
    /// 对方正在拖过来的东西。
    remote: Option<Remote>,
    /// 最近一次发给对方的提示，和发出的时刻。
    sent: Option<DropHint>,
    sent_at: Instant,
    /// 有一点小变化还没发，等到这个时刻再发。
    hint_due: Option<Instant>,
    /// 浮层窗口是否在屏幕上。
    shown: bool,
    /// 动画还在动：继续按帧画。
    moving: bool,
    /// 内容变了：下一帧要重画，即使动画已经停下。
    dirty: bool,
}

impl<O: Overlay> Worker<O> {
    async fn run(mut self) {
        let mut last = Instant::now();
        loop {
            let wake = if self.moving {
                Some(Instant::now() + FRAME)
            } else {
                self.deadline()
            };
            tokio::select! {
                changed = self.pointer.changed() => {
                    if changed.is_err() {
                        return;
                    }
                    self.on_pointer();
                }
                changed = self.edge.changed() => {
                    if changed.is_err() {
                        return;
                    }
                }
                changed = self.incoming.changed() => {
                    if changed.is_err() {
                        return;
                    }
                    self.on_remote();
                }
                _ = self.accent.changed() => self.dirty = true,
                Some(update) = self.transfers.recv() => self.on_transfer(update),
                Some(event) = self.events.recv() => self.on_event(event).await,
                _ = sleep_or_never(wake) => {}
            }
            let now = Instant::now();
            self.expire(now);
            self.watch();
            self.decide();
            self.publish_hint();
            let dt = (now - last).as_secs_f32();
            last = now;
            self.draw(dt);
        }
    }

    /// 下一个到期的提示或卡片。
    fn deadline(&self) -> Option<Instant> {
        let brief = self.brief.as_ref().map(|(_, _, until)| *until);
        let card = self.card.as_ref().map(|c| c.until);
        let remote = self.remote.as_ref().and_then(|r| r.until);
        brief
            .into_iter()
            .chain(card)
            .chain(remote)
            .chain(self.hint_due)
            .min()
    }

    fn expire(&mut self, now: Instant) {
        if self
            .brief
            .as_ref()
            .is_some_and(|(_, _, until)| *until <= now)
        {
            self.brief = None;
        }
        if self.card.as_ref().is_some_and(|c| c.until <= now) {
            self.card = None;
        }
        if self
            .remote
            .as_ref()
            .and_then(|r| r.until)
            .is_some_and(|until| until <= now)
        {
            self.remote = None;
        }
    }

    fn on_remote(&mut self) {
        let hint = self.incoming.borrow_and_update().clone();
        match hint {
            None => self.remote = None,
            Some(hint) if hint.state == HintState::Gone => {
                if let Some(remote) = &mut self.remote
                    && remote.until.is_none()
                {
                    let linger = if remote.hint.state == HintState::Over {
                        DROP_GRACE
                    } else {
                        GRACE
                    };
                    remote.until = Some(Instant::now() + linger);
                }
            }
            Some(hint) => self.remote = Some(Remote { hint, until: None }),
        }
    }

    fn on_pointer(&mut self) {
        let pointer = *self.pointer.borrow_and_update();
        match (&mut self.drag, pointer.held) {
            (None, true) => {
                self.drag = Some(Drag {
                    count: self.overlay.drag_count(),
                    files: None,
                    over: false,
                    summary: None,
                });
            }
            (Some(_), false) => self.drag = None,
            (Some(drag), true) => {
                // 靠近边缘时才去看拖的是什么，平时不打扰。
                let near = self
                    .edge
                    .borrow()
                    .as_ref()
                    .is_some_and(|edge| distance(edge, &pointer) <= NEAR);
                if near && drag.files.is_none() && self.overlay.drag_count() != drag.count {
                    let files = self.overlay.dragged_files();
                    drag.summary = summarize(&files);
                    drag.files = Some(files);
                }
            }
            (None, false) => {}
        }
    }

    async fn on_event(&mut self, event: OverlayEvent) {
        match event {
            OverlayEvent::DragEntered { files } => {
                if let Some(drag) = &mut self.drag {
                    drag.over = files;
                    // 有的系统（Windows）要到拖进接收条才知道拖的是什么。
                    if drag.files.is_none() {
                        let files = if files {
                            self.overlay.dragged_files()
                        } else {
                            Vec::new()
                        };
                        drag.summary = summarize(&files);
                        drag.files = Some(files);
                    }
                }
            }
            OverlayEvent::DragExited => {
                if let Some(drag) = &mut self.drag {
                    drag.over = false;
                }
            }
            OverlayEvent::Dropped { paths } => {
                let along = self.along();
                self.drag = None;
                match self.files.send(paths).await {
                    Ok(_) => {
                        // 进度到来之前先占住位置，免得胶囊收起又弹出来。
                        self.brief = None;
                        self.job = None;
                        self.pending_along = Some(along);
                    }
                    Err(e) => {
                        let look =
                            wide(Avatar::Warn, ["发不出去", &e.to_string(), ""], Action::None);
                        self.brief = Some((look, along, Instant::now() + NOTICE));
                    }
                }
            }
            OverlayEvent::Clicked(id) => self.on_click(id),
        }
    }

    fn on_click(&mut self, id: u32) {
        match id {
            hit::CANCEL => {
                if let Some(job) = &self.job {
                    self.files.cancel(job.update.id);
                }
            }
            hit::OPEN | hit::REVEAL | hit::CLOSE => {
                let Some(card) = self.card.take() else {
                    return;
                };
                if let Some(first) = card.saved.first() {
                    match id {
                        hit::OPEN => self.overlay.open(first),
                        hit::REVEAL => self.overlay.reveal(first),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn on_transfer(&mut self, update: TransferUpdate) {
        let along = self.transfer_along(update.id);
        let what = name_of(&update);
        let peer = self.peer();
        match &update.state {
            TransferState::Waiting | TransferState::Running => {
                self.card = None;
                self.brief = None;
                if !update.outgoing {
                    // 传输开始了，接收胶囊换成进度。
                    self.remote = None;
                }
                self.job = Some(Job { update, along });
                return;
            }
            TransferState::Done { saved } if !update.outgoing => {
                let look = Look::Card {
                    from: format!("来自 {peer}"),
                    name: what.clone(),
                    detail: format!("已保存到「下载」· {}", size_text(update.bytes)),
                    tile: tile_of(&update, saved),
                    chip: false,
                };
                self.card = Some(Card {
                    saved: saved.clone(),
                    look,
                    along,
                    until: Instant::now() + CARD,
                });
            }
            TransferState::Done { .. } => {
                let look = wide(
                    Avatar::Check,
                    ["已发送", &format!("到 {peer}"), ""],
                    Action::None,
                );
                self.brief = Some((look, along, Instant::now() + BRIEF));
            }
            TransferState::Interrupted => {
                let look = wide(
                    Avatar::Warn,
                    ["传输中断", &what, "重新连上后接着传"],
                    Action::None,
                );
                self.brief = Some((look, along, Instant::now() + NOTICE));
            }
            TransferState::Cancelled { .. } => {
                let detail = if update.outgoing {
                    "没有发送"
                } else {
                    "临时文件已删除"
                };
                let look = wide(Avatar::Cross, ["已取消", detail, ""], Action::None);
                self.brief = Some((look, along, Instant::now() + BRIEF));
            }
            TransferState::Failed { reason } => {
                let look = wide(Avatar::Warn, ["传输失败", &what, reason], Action::None);
                self.brief = Some((look, along, Instant::now() + NOTICE));
            }
        }
        if self
            .job
            .as_ref()
            .is_some_and(|job| job.update.id == update.id)
        {
            self.job = None;
        }
    }

    /// 拖着还不知道是什么的东西靠近边缘时，请平台在贴边留意；知道了或离开了就不再留意。
    fn watch(&mut self) {
        let pointer = *self.pointer.borrow();
        let edge = self.edge.borrow().clone();
        let wanted = match (&self.drag, &edge) {
            (Some(drag), Some(edge))
                if pointer.here && drag.files.is_none() && distance(edge, &pointer) <= NEAR =>
            {
                Some(strip_of(edge))
            }
            _ => None,
        };
        if wanted != self.watching {
            self.overlay.watch_edge(wanted);
            self.watching = wanted;
        }
    }

    /// 根据现状决定胶囊该显示成什么样子、放在哪里。
    fn decide(&mut self) {
        let edge = self.edge.borrow().clone();
        let Some(edge) = edge else {
            self.retarget(None);
            return;
        };
        let pointer = *self.pointer.borrow();
        let wanted = self
            .dragging(&edge, &pointer)
            .or_else(|| {
                self.job
                    .as_ref()
                    .map(|job| (progress(&job.update), job.along))
            })
            .or_else(|| self.receiving(&edge))
            .or_else(|| {
                self.brief
                    .as_ref()
                    .map(|(look, along, _)| (look.clone(), *along))
            })
            .or_else(|| {
                self.card
                    .as_ref()
                    .map(|card| (card.look.clone(), card.along))
            });
        let target = wanted.map(|(look, along)| {
            let frame = place(&look, edge.side, edge.screen, along, edge.scale);
            (look, frame)
        });
        self.retarget(target);
    }

    /// 拖着文件靠近边缘时的样子。
    fn dragging(&self, edge: &Edge, pointer: &Pointer) -> Option<(Look, f32)> {
        let drag = self.drag.as_ref()?;
        let files = drag.files.as_ref().filter(|f| !f.is_empty())?;
        if !pointer.here {
            return None;
        }
        let along = along_of(edge, pointer);
        let offline = !edge.online;
        if drag.over {
            let lines = if offline {
                ["未连接".into(), edge.peer.clone(), "连上后才能发送".into()]
            } else {
                ["松手发送到".into(), edge.peer.clone(), files_text(files)]
            };
            let look = Look::Wide {
                avatar: Avatar::Device(peer_glyph()),
                lines,
                hot: !offline,
                offline,
                action: Action::None,
            };
            return Some((look, along));
        }
        let d = distance(edge, pointer);
        if d > NEAR {
            return None;
        }
        let closeness = ((NEAR - d) / (NEAR - CLOSE)).clamp(0.0, 1.0);
        let look = Look::Tall {
            glyph: peer_glyph(),
            sending: true,
            offline,
            closeness,
        };
        Some((look, along))
    }

    /// 对方正在拖过来时的接收胶囊。
    fn receiving(&self, edge: &Edge) -> Option<(Look, f32)> {
        let hint = &self.remote.as_ref()?.hint;
        let along = along_from(edge, hint.along);
        let look = match hint.state {
            HintState::Over => {
                let what = if hint.items > 1 {
                    format!("{} 等 {} 项", hint.name, hint.items)
                } else {
                    hint.name.clone()
                };
                let detail = if hint.bytes > 0 {
                    format!("{what} · {}", size_text(hint.bytes))
                } else {
                    what
                };
                Look::Wide {
                    avatar: Avatar::Device(peer_glyph()),
                    lines: [format!("来自 {}", edge.peer), "松手即接收".into(), detail],
                    hot: true,
                    offline: false,
                    action: Action::None,
                }
            }
            _ => Look::Tall {
                glyph: peer_glyph(),
                sending: false,
                offline: false,
                closeness: f32::from(hint.closeness) / 100.0,
            },
        };
        Some((look, along))
    }

    /// 把本机拖动的现状告诉对方。只在有明显变化时更新，免得占用输入连接。
    fn publish_hint(&mut self) {
        let edge = self.edge.borrow().clone();
        let pointer = *self.pointer.borrow();
        let hint = edge.filter(|e| e.online).and_then(|edge| {
            let drag = self.drag.as_ref()?;
            let (name, items, bytes) = drag.summary.clone()?;
            if !pointer.here {
                return None;
            }
            let d = distance(&edge, &pointer);
            let (state, closeness) = if drag.over {
                (HintState::Over, 100)
            } else if d <= NEAR {
                let k = ((NEAR - d) / (NEAR - CLOSE)).clamp(0.0, 1.0);
                (HintState::Near, (k * 100.0).round() as u8)
            } else {
                return None;
            };
            Some(DropHint {
                state,
                along: fraction_of(&edge, &pointer),
                closeness,
                name,
                items,
                bytes,
            })
        });
        // 状态变了马上发；只是挪了一点，攒够间隔再发。
        let (changed, urgent) = match (&self.sent, &hint) {
            (None, None) => (false, false),
            (Some(a), Some(b)) => {
                let urgent = a.state != b.state || a.name != b.name;
                let moved =
                    a.closeness.abs_diff(b.closeness) >= 5 || a.along.abs_diff(b.along) >= 50;
                (urgent || moved, urgent)
            }
            _ => (true, true),
        };
        if !changed {
            self.hint_due = None;
            return;
        }
        let now = Instant::now();
        let due = self.sent_at + HINT_GAP;
        if urgent || now >= due {
            self.outgoing.send_replace(hint.clone());
            self.sent = hint;
            self.sent_at = now;
            self.hint_due = None;
        } else {
            self.hint_due = Some(due);
        }
    }

    fn retarget(&mut self, target: Option<(Look, Frame)>) {
        if target == self.target {
            return;
        }
        let side = self.edge.borrow().as_ref().map_or(Side::Left, |e| e.side);
        match &target {
            Some((look, frame)) => self.capsule.show(look.clone(), side, *frame),
            None => self.capsule.hide(),
        }
        self.target = target;
        self.moving = true;
        self.dirty = true;
    }

    fn draw(&mut self, dt: f32) {
        let moving = self.capsule.step(dt);
        let push = moving || self.dirty || !self.shown;
        self.moving = moving;
        self.dirty = false;
        let scale = self.edge.borrow().as_ref().map_or(1.0, |e| e.scale);
        let accent = *self.accent.borrow();
        match self.capsule.render(scale, accent) {
            Some((window, scene)) => {
                if push {
                    self.overlay.show(window, scale, scene);
                    self.shown = true;
                }
            }
            None if self.shown => {
                self.overlay.hide();
                self.shown = false;
            }
            None => {}
        }
    }

    fn along(&self) -> f32 {
        let pointer = *self.pointer.borrow();
        self.edge
            .borrow()
            .as_ref()
            .map_or(pointer.y, |edge| along_of(edge, &pointer))
    }

    /// 一次传输的胶囊放在哪里：沿用已有的位置，新的接收放在屏幕中间。
    fn transfer_along(&mut self, id: TransferId) -> f32 {
        if let Some(job) = self.job.as_ref().filter(|j| j.update.id == id) {
            return job.along;
        }
        if let Some(along) = self.pending_along.take() {
            return along;
        }
        // 对方刚才拖过来的位置。
        if let Some(remote) = &self.remote
            && let Some(edge) = self.edge.borrow().as_ref()
        {
            return along_from(edge, remote.hint.along);
        }
        self.edge
            .borrow()
            .as_ref()
            .map_or(0.0, |edge| match edge.side {
                Side::Left | Side::Right => edge.screen.y + edge.screen.h / 2.0,
                Side::Top | Side::Bottom => edge.screen.x + edge.screen.w / 2.0,
            })
    }

    fn peer(&self) -> String {
        self.edge
            .borrow()
            .as_ref()
            .map_or_else(|| "对方".to_owned(), |e| e.peer.clone())
    }
}

/// 光标离朝向对方的那条边多远（点）。
fn distance(edge: &Edge, p: &Pointer) -> f32 {
    let s = edge.screen;
    let d = match edge.side {
        Side::Left => p.x - s.x,
        Side::Right => s.right() - 1.0 - p.x,
        Side::Top => p.y - s.y,
        Side::Bottom => s.bottom() - 1.0 - p.y,
    };
    d.max(0.0) / edge.scale.max(0.01)
}

/// 贴着朝向对方那条边的一窄条（桌面坐标）。
fn strip_of(edge: &Edge) -> Frame {
    let s = edge.screen;
    let w = STRIP * edge.scale;
    match edge.side {
        Side::Left => Frame::new(s.x, s.y, w, s.h),
        Side::Right => Frame::new(s.right() - w, s.y, w, s.h),
        Side::Top => Frame::new(s.x, s.y, s.w, w),
        Side::Bottom => Frame::new(s.x, s.bottom() - w, s.w, w),
    }
}

/// 告诉对方“没在拖了”的提示。
pub(crate) fn gone() -> DropHint {
    DropHint {
        state: HintState::Gone,
        along: 0,
        closeness: 0,
        name: String::new(),
        items: 0,
        bytes: 0,
    }
}

/// 沿边位置换成提示里的比例（0–10000）。
fn fraction_of(edge: &Edge, p: &Pointer) -> u16 {
    let s = edge.screen;
    let k = match edge.side {
        Side::Left | Side::Right => (p.y - s.y) / s.h.max(1.0),
        Side::Top | Side::Bottom => (p.x - s.x) / s.w.max(1.0),
    };
    (k.clamp(0.0, 1.0) * ALONG_FULL).round() as u16
}

/// 提示里的比例换回本机沿边的坐标。
fn along_from(edge: &Edge, fraction: u16) -> f32 {
    let k = f32::from(fraction.min(10_000)) / ALONG_FULL;
    let s = edge.screen;
    match edge.side {
        Side::Left | Side::Right => s.y + k * s.h,
        Side::Top | Side::Bottom => s.x + k * s.w,
    }
}

/// 拖着的东西的摘要：第一项的名字、几项、总大小（含文件夹时不知道总大小，记为 0）。
fn summarize(files: &[PathBuf]) -> Option<(String, u32, u64)> {
    let first = files.first()?;
    let name = first.file_name()?.to_string_lossy().into_owned();
    let mut bytes = 0u64;
    for file in files {
        match std::fs::metadata(file) {
            Ok(meta) if meta.is_file() => bytes += meta.len(),
            _ => {
                bytes = 0;
                break;
            }
        }
    }
    Some((name, u32::try_from(files.len()).unwrap_or(u32::MAX), bytes))
}

fn along_of(edge: &Edge, p: &Pointer) -> f32 {
    match edge.side {
        Side::Left | Side::Right => p.y,
        Side::Top | Side::Bottom => p.x,
    }
}

/// 对方是什么设备。还没有从协议拿到对方的系统，暂按“另一种系统”画：Mac 上画显示器，Windows 上画笔记本。
fn peer_glyph() -> Glyph {
    if std::env::consts::OS == "macos" {
        Glyph::Monitor
    } else {
        Glyph::Laptop
    }
}

fn wide(avatar: Avatar, lines: [&str; 3], action: Action) -> Look {
    Look::Wide {
        avatar,
        lines: lines.map(str::to_owned),
        hot: false,
        offline: false,
        action,
    }
}

/// 传输中的样子：进度环、百分比、名字、已传多少。
fn progress(update: &TransferUpdate) -> Look {
    let fraction = if update.bytes == 0 {
        if matches!(update.state, TransferState::Running) {
            1.0
        } else {
            0.0
        }
    } else {
        update.done as f32 / update.bytes as f32
    };
    let pct = (fraction * 100.0).round() as u32;
    let verb = if update.outgoing {
        "发送中"
    } else {
        "接收中"
    };
    let amount = format!("{} / {}", size_text(update.done), size_text(update.bytes));
    Look::Wide {
        avatar: Avatar::Ring(fraction),
        lines: [format!("{verb} {pct}%"), name_of(update), amount],
        hot: false,
        offline: false,
        action: Action::Cancel,
    }
}

fn name_of(update: &TransferUpdate) -> String {
    if update.items > 1 {
        format!("{} 等 {} 项", update.name, update.items)
    } else {
        update.name.clone()
    }
}

/// 卡片上的小图标：多项、文件夹，或者带扩展名的文件。
fn tile_of(update: &TransferUpdate, saved: &[PathBuf]) -> Tile {
    if update.items > 1 {
        return Tile::Stack(update.items);
    }
    if saved.first().is_some_and(|p| p.is_dir()) {
        return Tile::Folder;
    }
    let ext = std::path::Path::new(&update.name)
        .extension()
        .map(|e| e.to_string_lossy().to_uppercase())
        .unwrap_or_default();
    let ext: String = ext.chars().take(4).collect();
    let tone = match ext.as_str() {
        "PDF" => Rgba::hex(0xC2410C, 255),
        "MOV" | "MP4" | "MKV" | "AVI" => Rgba::hex(0x6A4BD8, 255),
        "PNG" | "JPG" | "JPEG" | "GIF" | "HEIC" | "WEBP" | "SVG" => Rgba::hex(0x00796B, 255),
        "DOC" | "DOCX" | "TXT" | "MD" | "PAGE" => Rgba::hex(0x0A6CFF, 255),
        "ZIP" | "RAR" | "7Z" | "DMG" => Rgba::hex(0x8A6D1F, 255),
        _ => Rgba::hex(0x0F766E, 255),
    };
    Tile::Doc { ext, tone }
}

/// 拖着的文件的说明：一个时是名字，多个时是第一个的名字和数量。
fn files_text(files: &[PathBuf]) -> String {
    let first = files
        .first()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if files.len() > 1 {
        format!("{first} 等 {} 项", files.len())
    } else {
        first
    }
}

/// 文件大小，给人看的写法。
pub(crate) fn size_text(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if value < 10.0 {
        format!("{value:.1} {}", UNITS[unit])
    } else {
        format!("{value:.0} {}", UNITS[unit])
    }
}

async fn sleep_or_never(at: Option<Instant>) {
    match at {
        Some(at) => sleep_until(at).await,
        None => std::future::pending().await,
    }
}
