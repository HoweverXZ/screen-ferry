//! 输入注入：定义 `InputInject` trait。

use ferry_types::{InputEvent, LockKeys};

use crate::error::Result;

/// 向本机注入键鼠事件。
pub trait InputInject: Send {
    /// 注入一个事件。`Button` 与 `Wheel` 只执行按键和滚动，光标定位由调用方先行注入。
    fn inject(&mut self, event: &InputEvent) -> Result<()>;

    /// 把 `mask` 中的锁定键调整到与 `wanted` 一致，其余锁定键不动。
    fn sync_locks(&mut self, wanted: LockKeys, mask: LockKeys) -> Result<()>;

    /// 隐藏或重新显示本机光标。光标在对方屏幕上时隐藏，免得本机屏幕上还停着一个不动的光标；
    /// 本机用户自己动一下鼠标，光标就应当重新出现。不支持的平台什么也不做。
    fn set_cursor_hidden(&mut self, hidden: bool) -> Result<()> {
        let _ = hidden;
        Ok(())
    }
}
