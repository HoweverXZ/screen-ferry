//! 系统权限：`Permissions` 的 macOS 实现。
//!
//! 拦截和注入键鼠需要"辅助功能"权限。从终端启动时，权限授予的是终端程序。

use ferry_platform::{Error, Permissions, Result};

pub(crate) const ACCESSIBILITY: &str = "需要“辅助功能”权限：请在「系统设置 → 隐私与安全性 → 辅助功能」中允许 ScreenFerry（从终端运行时是终端程序）";

use std::ffi::c_void;

/// CoreFoundation 结构体，只取地址用。
#[repr(C)]
struct Opaque {
    _private: [u8; 0],
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    /// 当前进程是否已获得辅助功能权限。返回 C 的 `Boolean`。
    safe fn AXIsProcessTrusted() -> u8;
    /// 同上；选项中带 `kAXTrustedCheckOptionPrompt = true` 时，未授权会弹出系统提示并把本程序加入列表。
    fn AXIsProcessTrustedWithOptions(options: *const c_void) -> u8;
    static kAXTrustedCheckOptionPrompt: *const c_void;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFBooleanTrue: *const c_void;
    static kCFTypeDictionaryKeyCallBacks: Opaque;
    static kCFTypeDictionaryValueCallBacks: Opaque;
    fn CFDictionaryCreate(
        allocator: *const c_void,
        keys: *const *const c_void,
        values: *const *const c_void,
        count: isize,
        key_callbacks: *const Opaque,
        value_callbacks: *const Opaque,
    ) -> *const c_void;
    fn CFRelease(cf: *const c_void);
}

/// 本机权限检查。
#[derive(Clone, Copy, Debug, Default)]
pub struct MacPermissions;

impl Permissions for MacPermissions {
    fn check_input(&self) -> Result<()> {
        if AXIsProcessTrusted() != 0 {
            Ok(())
        } else {
            Err(Error::PermissionDenied(ACCESSIBILITY))
        }
    }

    fn request_input(&self) -> Result<()> {
        // SAFETY: 键值都是系统提供的常量 CF 对象；字典用完即释放。
        let trusted = unsafe {
            let keys = [kAXTrustedCheckOptionPrompt];
            let values = [kCFBooleanTrue];
            let options = CFDictionaryCreate(
                std::ptr::null(),
                keys.as_ptr(),
                values.as_ptr(),
                1,
                &raw const kCFTypeDictionaryKeyCallBacks,
                &raw const kCFTypeDictionaryValueCallBacks,
            );
            let trusted = AXIsProcessTrustedWithOptions(options);
            if !options.is_null() {
                CFRelease(options);
            }
            trusted
        };
        if trusted != 0 {
            Ok(())
        } else {
            Err(Error::PermissionDenied(ACCESSIBILITY))
        }
    }
}
