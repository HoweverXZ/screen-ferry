//! 拖放目标：实现 OLE 的 `IDropTarget`，接住拖到浮层上的文件。
//!
//! 只接文件（`CF_HDROP`）：拖进来的是文件时鼠标显示“复制”，放下后把路径交给调用方；
//! 别的内容显示“不可放”，并告诉调用方这次拖的不是文件。

use std::path::PathBuf;
use std::sync::Arc;

use ferry_platform::OverlayEvent;
use windows::Win32::Foundation::POINTL;
use windows::Win32::System::Com::{DVASPECT_CONTENT, FORMATETC, IDataObject, TYMED_HGLOBAL};
use windows::Win32::System::Ole::{
    CF_HDROP, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_NONE, IDropTarget, IDropTarget_Impl,
    ReleaseStgMedium,
};
use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};
use windows_core::{Ref, implement};

use super::{Shared, lock};

#[implement(IDropTarget)]
pub(super) struct DropTarget {
    shared: Arc<Shared>,
}

impl DropTarget {
    pub(super) fn new(shared: Arc<Shared>) -> Self {
        Self { shared }
    }
}

impl IDropTarget_Impl for DropTarget_Impl {
    fn DragEnter(
        &self,
        data: Ref<'_, IDataObject>,
        _keys: MODIFIERKEYS_FLAGS,
        _at: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows_core::Result<()> {
        let files = data.ok().map(file_paths).unwrap_or_default();
        let any = !files.is_empty();
        {
            let mut drag = lock(&self.shared.drag);
            drag.holding_files = any;
            if any {
                drag.count += 1;
                drag.files = files;
            }
        }
        self.shared.emit(OverlayEvent::DragEntered { files: any });
        set(effect, any);
        Ok(())
    }

    fn DragOver(
        &self,
        _keys: MODIFIERKEYS_FLAGS,
        _at: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows_core::Result<()> {
        set(effect, lock(&self.shared.drag).holding_files);
        Ok(())
    }

    fn DragLeave(&self) -> windows_core::Result<()> {
        lock(&self.shared.drag).holding_files = false;
        self.shared.emit(OverlayEvent::DragExited);
        Ok(())
    }

    fn Drop(
        &self,
        data: Ref<'_, IDataObject>,
        _keys: MODIFIERKEYS_FLAGS,
        _at: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows_core::Result<()> {
        let paths = data.ok().map(file_paths).unwrap_or_default();
        lock(&self.shared.drag).holding_files = false;
        let any = !paths.is_empty();
        if any {
            self.shared.emit(OverlayEvent::Dropped { paths });
        }
        set(effect, any);
        Ok(())
    }
}

/// 告诉拖动的来源这里能不能放。
fn set(effect: *mut DROPEFFECT, files: bool) {
    if !effect.is_null() {
        // SAFETY: 系统传来的指针非空时指向有效的 DROPEFFECT。
        unsafe {
            *effect = if files {
                DROPEFFECT_COPY
            } else {
                DROPEFFECT_NONE
            }
        };
    }
}

/// 取出拖动数据里的文件和文件夹路径；不是文件时为空。
fn file_paths(data: &IDataObject) -> Vec<PathBuf> {
    let format = FORMATETC {
        cfFormat: CF_HDROP.0,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    };
    // SAFETY: 格式描述是有效的局部值；取到的存储介质用完后释放。
    unsafe {
        let Ok(mut medium) = data.GetData(&format) else {
            return Vec::new();
        };
        let drop = HDROP(medium.u.hGlobal.0);
        let count = DragQueryFileW(drop, u32::MAX, None);
        let mut paths = Vec::with_capacity(count as usize);
        for i in 0..count {
            let len = DragQueryFileW(drop, i, None) as usize;
            let mut buf = vec![0u16; len + 1];
            let got = DragQueryFileW(drop, i, Some(&mut buf)) as usize;
            if got > 0 {
                paths.push(PathBuf::from(String::from_utf16_lossy(&buf[..got])));
            }
        }
        ReleaseStgMedium(&mut medium);
        paths
    }
}
