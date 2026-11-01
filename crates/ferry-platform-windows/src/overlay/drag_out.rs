//! 从浮层把文件拖出去：OLE 的拖放来源。
//!
//! 数据用资源管理器自己的文件数据对象（`SHCreateDataObject`），拖到文件夹、桌面或别的程序里，
//! 都和从资源管理器里拖出来一样；跟着鼠标的文件图标也交给系统生成。
//! 拖放期间系统接管消息循环，界面线程上的其他消息照常处理。

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use ferry_platform::OverlayEvent;
use windows::Win32::Foundation::{
    DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, DRAGDROP_S_USEDEFAULTCURSORS, POINT, S_OK,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, IDataObject};
use windows::Win32::System::Ole::{
    DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_MOVE, DoDragDrop, IDropSource, IDropSource_Impl,
};
use windows::Win32::System::SystemServices::{MK_LBUTTON, MODIFIERKEYS_FLAGS};
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    CLSID_DragDropHelper, IDragSourceHelper, ILCreateFromPathW, ILFindLastID, ILFree,
    SHCreateDataObject,
};
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
use windows::core::{BOOL, HRESULT, HSTRING, IUnknown};
use windows_core::implement;

use super::Shared;

/// 把这些文件拖出去，直到用户放下或取消。必须在浮层的界面线程上调用。
///
/// 由放下的地方决定移动还是复制（同一磁盘上默认移动）。
pub(super) fn drag_out(shared: &Shared, paths: &[PathBuf]) {
    let Some(data) = data_object(paths) else {
        return;
    };
    // SAFETY: 本线程已初始化 OLE；拖动图像助手是系统自带的组件，失败只是没有图标。
    unsafe {
        if let Ok(helper) = CoCreateInstance::<_, IDragSourceHelper>(
            &CLSID_DragDropHelper,
            None::<&IUnknown>,
            CLSCTX_INPROC_SERVER,
        ) {
            let mut at = POINT::default();
            let _ = GetCursorPos(&mut at);
            let _ = helper.InitializeFromWindow(None, Some(&at), &data);
        }
    }
    let source: IDropSource = DropSource.into();
    shared.dragging_out.store(true, Ordering::Release);
    shared.emit(OverlayEvent::DragOutStarted);
    let mut effect = DROPEFFECT::default();
    // SAFETY: 数据对象和拖放来源在调用期间一直存活，effect 指向有效的局部变量。
    let result = unsafe {
        DoDragDrop(
            &data,
            &source,
            DROPEFFECT_COPY | DROPEFFECT_MOVE,
            &mut effect,
        )
    };
    shared.dragging_out.store(false, Ordering::Release);
    shared.emit(OverlayEvent::DragOutEnded {
        dropped: result == DRAGDROP_S_DROP,
    });
}

/// 资源管理器的文件数据对象。这些文件都在同一个文件夹（“下载”）里；不在的略过。
fn data_object(paths: &[PathBuf]) -> Option<IDataObject> {
    let parent = paths.first()?.parent()?;
    let items: Vec<&Path> = paths
        .iter()
        .map(PathBuf::as_path)
        .filter(|p| p.parent() == Some(parent))
        .collect();
    // SAFETY: 路径字符串在调用期间有效；创建的 ID 列表用完即释放，数据对象自己保存一份。
    unsafe {
        let folder = ILCreateFromPathW(&HSTRING::from(parent));
        if folder.is_null() {
            return None;
        }
        let absolute: Vec<*mut ITEMIDLIST> = items
            .iter()
            .map(|p| ILCreateFromPathW(&HSTRING::from(*p)))
            .filter(|p| !p.is_null())
            .collect();
        let children: Vec<*const ITEMIDLIST> = absolute
            .iter()
            .map(|p| ILFindLastID(*p).cast_const())
            .collect();
        let data = if children.is_empty() {
            None
        } else {
            SHCreateDataObject::<_, IDataObject>(
                Some(folder.cast_const()),
                Some(&children),
                None::<&IDataObject>,
            )
            .ok()
        };
        for p in absolute {
            ILFree(Some(p.cast_const()));
        }
        ILFree(Some(folder.cast_const()));
        data
    }
}

/// 拖放来源：松开左键即放下，按 Esc 取消，鼠标样子用系统默认的。
#[implement(IDropSource)]
struct DropSource;

impl IDropSource_Impl for DropSource_Impl {
    fn QueryContinueDrag(&self, escape: BOOL, keys: MODIFIERKEYS_FLAGS) -> HRESULT {
        if escape.as_bool() {
            DRAGDROP_S_CANCEL
        } else if keys.0 & MK_LBUTTON.0 == 0 {
            DRAGDROP_S_DROP
        } else {
            S_OK
        }
    }

    fn GiveFeedback(&self, _effect: DROPEFFECT) -> HRESULT {
        DRAGDROP_S_USEDEFAULTCURSORS
    }
}
