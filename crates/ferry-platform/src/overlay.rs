//! 屏幕边缘的浮层（边缘投递胶囊）：定义 `Overlay` trait。
//!
//! 浮层是一个置顶、不抢焦点的透明窗口，照着 [`Scene`] 画出胶囊，接收拖进来的文件和点击。
//! 画什么、放在哪里都由调用方决定；实现只负责画、摆放和转告发生的事。
//! 窗口在显示时整块都接收鼠标和拖放，所以它要一直延伸到屏幕边缘：把文件推到边缘就能放下。

use std::path::{Path, PathBuf};

use ferry_types::{Frame, Scene};

use crate::error::Result;

/// 浮层上发生的事。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OverlayEvent {
    /// 拖着东西进入了浮层。`files` 表示拖的是不是文件。
    DragEntered { files: bool },
    /// 拖着的东西离开了浮层，没有放下。
    DragExited,
    /// 在浮层上放下了这些文件和文件夹。
    Dropped { paths: Vec<PathBuf> },
    /// 点了 [`Scene::hits`] 中编号为它的那一块。
    Clicked(u32),
}

/// 接收浮层事件的回调。在系统的界面线程上调用，必须立即返回。
pub type OverlaySink = Box<dyn FnMut(OverlayEvent) + Send>;

/// 屏幕边缘的浮层。可以在任意线程调用，实现自己把工作交给界面线程。
pub trait Overlay: Send {
    /// 开始工作，之后发生的事交给 `sink`。
    fn start(&mut self, sink: OverlaySink) -> Result<()>;

    /// 显示或更新浮层：窗口放在桌面坐标 `frame`（左上角原点，单位与显示器信息一致），
    /// 内容是 `scene`，其中每个点对应 `scale` 个桌面单位。
    fn show(&mut self, frame: Frame, scale: f32, scene: Scene);

    /// 收起浮层。
    fn hide(&mut self);

    /// 拖放用的剪贴板的变化计数。用户开始拖新的东西时会变，借此判断“这次拖的是什么”。
    fn drag_count(&self) -> i64;

    /// 正在拖的文件和文件夹；拖的不是文件时为空。
    fn dragged_files(&self) -> Vec<PathBuf>;

    /// 拖着东西靠近朝向对方的边时，留意贴边的这一条区域 `strip`（桌面坐标）；`None` 为不再留意。
    ///
    /// 有的系统（Windows）没法提前知道别的程序在拖什么，只能在这里放一条看不见的接收条：
    /// 拖进来的是文件，[`drag_count`](Self::drag_count) 和 [`dragged_files`](Self::dragged_files)
    /// 随之更新，并发出 [`OverlayEvent::DragEntered`]。能提前知道的系统什么也不用做。
    fn watch_edge(&mut self, strip: Option<Frame>) {
        let _ = strip;
    }

    /// 用默认程序打开。
    fn open(&self, path: &Path);

    /// 在访达或资源管理器里显示并选中。
    fn reveal(&self, path: &Path);
}
