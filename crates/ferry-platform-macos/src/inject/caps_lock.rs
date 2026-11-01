//! Caps Lock 的锁定状态。
//!
//! 程序发出的 Caps Lock 按键改不了大小写状态，指示灯也不会亮，只能经 IOKit 直接读写
//! 系统的锁定状态。这几个函数在 IOKit 里，objc2 没有绑定，在这里声明。

use std::ffi::{c_char, c_void};

use ferry_platform::{Error, Result};

type KernReturn = i32;
type MachPort = u32;

const KERN_SUCCESS: KernReturn = 0;
/// `kIOHIDParamConnectType`
const PARAM_CONNECT_TYPE: u32 = 1;
/// `kIOHIDCapsLockState`
const CAPS_LOCK_STATE: i32 = 1;

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOServiceMatching(name: *const c_char) -> *mut c_void;
    fn IOServiceGetMatchingService(main_port: MachPort, matching: *mut c_void) -> MachPort;
    fn IOServiceOpen(
        service: MachPort,
        owning_task: MachPort,
        kind: u32,
        connect: *mut MachPort,
    ) -> KernReturn;
    fn IOServiceClose(connect: MachPort) -> KernReturn;
    fn IOObjectRelease(object: MachPort) -> KernReturn;
    fn IOHIDGetModifierLockState(handle: MachPort, selector: i32, state: *mut bool) -> KernReturn;
    fn IOHIDSetModifierLockState(handle: MachPort, selector: i32, state: bool) -> KernReturn;
}

unsafe extern "C" {
    /// 本进程的 task 端口，即 C 里的 `mach_task_self()`。
    static mach_task_self_: MachPort;
}

/// 与系统 HID 服务的一条连接。
pub(super) struct CapsLock {
    connect: MachPort,
}

impl CapsLock {
    pub(super) fn open() -> Result<Self> {
        // SAFETY: 参数是以 NUL 结尾的类名；`IOServiceGetMatchingService` 接管匹配字典的所有权。
        let service = unsafe {
            let matching = IOServiceMatching(c"IOHIDSystem".as_ptr());
            if matching.is_null() {
                return Err(os("IOServiceMatching", 0));
            }
            IOServiceGetMatchingService(0, matching)
        };
        if service == 0 {
            return Err(os("IOServiceGetMatchingService", 0));
        }
        let mut connect = 0;
        // SAFETY: `service` 是刚取得的有效对象，用完即释放；`connect` 指向有效的缓冲区。
        let kr = unsafe {
            let kr = IOServiceOpen(service, mach_task_self_, PARAM_CONNECT_TYPE, &mut connect);
            IOObjectRelease(service);
            kr
        };
        if kr != KERN_SUCCESS {
            return Err(os("IOServiceOpen", kr));
        }
        Ok(Self { connect })
    }

    /// Caps Lock 此刻是否打开。
    pub(super) fn get(&self) -> Result<bool> {
        let mut on = false;
        // SAFETY: 连接在本对象存在期间有效，`on` 指向有效的缓冲区。
        let kr = unsafe { IOHIDGetModifierLockState(self.connect, CAPS_LOCK_STATE, &mut on) };
        if kr == KERN_SUCCESS {
            Ok(on)
        } else {
            Err(os("IOHIDGetModifierLockState", kr))
        }
    }

    pub(super) fn set(&self, on: bool) -> Result<()> {
        // SAFETY: 连接在本对象存在期间有效。
        let kr = unsafe { IOHIDSetModifierLockState(self.connect, CAPS_LOCK_STATE, on) };
        if kr == KERN_SUCCESS {
            Ok(())
        } else {
            Err(os("IOHIDSetModifierLockState", kr))
        }
    }
}

impl Drop for CapsLock {
    fn drop(&mut self) {
        // SAFETY: 连接由 `open` 打开，只关闭这一次。
        unsafe {
            IOServiceClose(self.connect);
        }
    }
}

fn os(call: &'static str, kr: KernReturn) -> Error {
    Error::Os {
        call,
        detail: format!("返回 {kr:#x}"),
    }
}
