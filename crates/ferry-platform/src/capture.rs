//! 输入捕获：定义 `InputCapture` trait。

use ferry_types::{KeyAction, KeyCode, MouseButton, Point};

use crate::error::Result;

/// 捕获到的一个本机输入。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureEvent {
    /// 鼠标移动：本机光标位置，以及本次的相对位移。抓取期间光标被固定，只有位移有意义。
    Pointer {
        position: Point,
        dx: i32,
        dy: i32,
    },
    Button {
        button: MouseButton,
        pressed: bool,
    },
    /// 滚轮，单位与方向见 [`ferry_types::InputEvent::Wheel`]。
    Wheel {
        dx: i32,
        dy: i32,
    },
    Key {
        code: KeyCode,
        action: KeyAction,
    },
}

/// 接收捕获事件的回调。在捕获线程上调用，必须立即返回。
pub type CaptureSink = Box<dyn FnMut(CaptureEvent) + Send>;

/// 本机键鼠捕获。
pub trait InputCapture: Send {
    /// 开始捕获，事件交给 `sink`，直到本对象被丢弃。
    fn start(&mut self, sink: CaptureSink) -> Result<()>;

    /// 抓取：本机不再响应键鼠，光标固定在原地，事件只交给 `sink`。
    fn grab(&self) -> Result<()>;

    /// 结束抓取，并把本机光标放到 `at`。
    fn release(&self, at: Point) -> Result<()>;
}
