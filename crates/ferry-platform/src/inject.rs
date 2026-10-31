//! 输入注入：定义 `InputInject` trait。

use ferry_types::{InputEvent, LockKeys};

use crate::error::Result;

/// 向本机注入键鼠事件。
pub trait InputInject: Send {
    /// 注入一个事件。`Button` 与 `Wheel` 只执行按键和滚动，光标定位由调用方先行注入。
    fn inject(&mut self, event: &InputEvent) -> Result<()>;

    /// 把 `mask` 中的锁定键调整到与 `wanted` 一致，其余锁定键不动。
    fn sync_locks(&mut self, wanted: LockKeys, mask: LockKeys) -> Result<()>;
}
