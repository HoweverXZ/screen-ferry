//! 显示器信息：定义 `Displays` trait。

use ferry_types::Display;

use crate::error::Result;

/// 查询本机显示器。
pub trait Displays: Send {
    /// 当前所有显示器，使用本机桌面坐标系。
    fn displays(&self) -> Result<Vec<Display>>;
}
