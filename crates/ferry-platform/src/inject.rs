//! 输入注入：定义 `InputInject` trait。

use ferry_types::InputEvent;

use crate::error::Result;

/// 向本机注入键鼠事件。
pub trait InputInject: Send {
    /// 注入一个事件。`Button` 与 `Wheel` 只执行按键和滚动，光标定位由调用方先行注入。
    fn inject(&mut self, event: &InputEvent) -> Result<()>;
}
