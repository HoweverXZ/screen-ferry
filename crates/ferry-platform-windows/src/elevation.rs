//! 本进程是不是以管理员权限运行的。
//!
//! 以管理员权限运行会带来一个看不见的坏处：Windows 的用户界面特权隔离（UIPI）不允许
//! 普通权限的程序把文件拖进管理员权限的窗口。资源管理器是普通权限，于是拖到屏幕边缘
//! 时既不出现接收条，也一直是“禁止”光标——用户完全不知道是权限的问题。
//!
//! 所以启动时查一次，是管理员权限就提示以普通权限重新启动。

use std::mem::size_of;

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// 当前进程是否以管理员权限运行。
///
/// 问不到的时候当作“不是”：这个检查只用来提示，不值得因为问不到就报错。
pub fn is_elevated() -> bool {
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut returned = 0_u32;
        let queried = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        )
        .is_ok();
        let _ = CloseHandle(token);
        queried && elevation.TokenIsElevated != 0
    }
}
