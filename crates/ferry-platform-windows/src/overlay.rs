//! 屏幕边缘浮层：`Overlay` 的 Windows 实现。
//!
//! 还没有做：暂时什么都不显示，也不接收拖放。Windows 上照样能用 `screenferry send` 发文件，
//! 收到的文件也照样放进“下载”文件夹。

use std::path::{Path, PathBuf};

use ferry_platform::{Overlay, OverlaySink, Result};
use ferry_types::{Frame, Scene};

/// 暂未实现的浮层。
#[derive(Debug, Default)]
pub struct WinOverlay;

impl WinOverlay {
    pub fn new() -> Self {
        Self
    }
}

impl Overlay for WinOverlay {
    fn start(&mut self, _sink: OverlaySink) -> Result<()> {
        Ok(())
    }

    fn show(&mut self, _frame: Frame, _scale: f32, _scene: Scene) {}

    fn hide(&mut self) {}

    fn drag_count(&self) -> i64 {
        0
    }

    fn dragged_files(&self) -> Vec<PathBuf> {
        Vec::new()
    }

    fn open(&self, _path: &Path) {}

    fn reveal(&self, _path: &Path) {}
}
