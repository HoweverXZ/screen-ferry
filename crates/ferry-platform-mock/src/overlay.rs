//! 屏幕边缘浮层：`Overlay` 的测试实现。

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use ferry_platform::{Overlay, OverlayEvent, OverlaySink, Result};
use ferry_types::{Frame, Scene};

/// 假的浮层：记下最后显示的内容，测试可以模拟拖入、放下和点击。
#[derive(Clone, Default)]
pub struct MockOverlay {
    state: Arc<Mutex<State>>,
}

#[derive(Default)]
struct State {
    sink: Option<OverlaySink>,
    /// 当前显示的窗口位置和内容；收起时为 `None`。
    shown: Option<(Frame, Scene)>,
    drag_count: i64,
    dragged: Vec<PathBuf>,
    watching: Option<Frame>,
    opened: Vec<PathBuf>,
    revealed: Vec<PathBuf>,
}

impl MockOverlay {
    pub fn new() -> Self {
        Self::default()
    }

    /// 当前显示的窗口位置和内容。
    pub fn shown(&self) -> Option<(Frame, Scene)> {
        self.lock().shown.clone()
    }

    /// 模拟用户开始拖这些文件（空表示拖的不是文件）。
    pub fn start_drag(&self, files: Vec<PathBuf>) {
        let mut state = self.lock();
        state.drag_count += 1;
        state.dragged = files;
    }

    /// 模拟浮层上发生了一件事。尚未开始工作时丢弃。
    pub fn emit(&self, event: OverlayEvent) {
        if let Some(sink) = self.lock().sink.as_mut() {
            sink(event);
        }
    }

    /// 正在留意的贴边区域。
    pub fn watching(&self) -> Option<Frame> {
        self.lock().watching
    }

    /// 请求打开过的文件。
    pub fn opened(&self) -> Vec<PathBuf> {
        self.lock().opened.clone()
    }

    /// 请求在文件夹中显示过的文件。
    pub fn revealed(&self) -> Vec<PathBuf> {
        self.lock().revealed.clone()
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Overlay for MockOverlay {
    fn start(&mut self, sink: OverlaySink) -> Result<()> {
        self.lock().sink = Some(sink);
        Ok(())
    }

    fn show(&mut self, frame: Frame, _scale: f32, scene: Scene) {
        self.lock().shown = Some((frame, scene));
    }

    fn hide(&mut self) {
        self.lock().shown = None;
    }

    fn drag_count(&self) -> i64 {
        self.lock().drag_count
    }

    fn dragged_files(&self) -> Vec<PathBuf> {
        self.lock().dragged.clone()
    }

    fn watch_edge(&mut self, strip: Option<Frame>) {
        self.lock().watching = strip;
    }

    fn open(&self, path: &Path) {
        self.lock().opened.push(path.to_owned());
    }

    fn reveal(&self, path: &Path) {
        self.lock().revealed.push(path.to_owned());
    }
}
