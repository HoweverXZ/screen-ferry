//! 屏幕边缘浮层：`Overlay` 的 macOS 实现。
//!
//! 一个无边框、不抢焦点、浮在最上层的 NSPanel，内容是自定义的 NSView：照着 [`Scene`]
//! 用 NSBezierPath 和 NSString 画出来，接收拖进来的文件（NSDraggingDestination）和点击；
//! 从可以拖动的区域把交来的文件拖出去（NSDraggingSource）。
//! 显示在所有桌面空间里，全屏应用之上也能看到。所有 AppKit 调用都经 GCD 主队列交给主线程。

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use dispatch2::DispatchQueue;
use ferry_platform::{Overlay, OverlayEvent, OverlaySink, Result};
use ferry_types::{Align, Frame, Rgba, Scene, Shape, Weight};
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{
    AnyThread, ClassType, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
};
use objc2_app_kit::{
    NSBackingStoreType, NSBezierPath, NSBitmapImageFileType, NSBitmapImageRep, NSColor,
    NSDragOperation, NSDraggingContext, NSDraggingDestination, NSDraggingInfo, NSDraggingItem,
    NSDraggingSession, NSDraggingSource, NSEvent, NSFont, NSFontAttributeName, NSFontWeightRegular,
    NSFontWeightSemibold, NSForegroundColorAttributeName, NSGraphicsContext, NSLineBreakMode,
    NSLineCapStyle, NSLineJoinStyle, NSMutableParagraphStyle, NSPanel,
    NSParagraphStyleAttributeName, NSPasteboard, NSPasteboardNameDrag, NSPasteboardTypeFileURL,
    NSPasteboardURLReadingFileURLsOnlyKey, NSResponder, NSScreen, NSStatusWindowLevel,
    NSStringDrawing, NSTextAlignment, NSView, NSWindowCollectionBehavior, NSWindowStyleMask,
    NSWorkspace,
};
use objc2_core_foundation::{CGFloat, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{
    CGBitmapContextCreate, CGBitmapContextCreateImage, CGColorSpace, CGContext, CGImageAlphaInfo,
};
use objc2_foundation::{NSArray, NSDictionary, NSNumber, NSString, NSURL};

/// 浮层事件的去处。整个进程只有一个浮层窗口。
static SINK: Mutex<Option<OverlaySink>> = Mutex::new(None);

fn emit(event: OverlayEvent) {
    let mut sink = SINK.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(sink) = sink.as_mut() {
        sink(event);
    }
}

struct ViewIvars {
    scene: RefCell<Scene>,
    /// 每个场景点对应多少个视图点。
    scale: Cell<f64>,
    /// 可以拖出去的文件。
    offered: RefCell<Vec<PathBuf>>,
    /// 在可以拖动的区域按下了鼠标：按下的位置（视图坐标），移动够远就开始拖。
    press: Cell<Option<CGPoint>>,
}

/// 按下后移动多远（点）才算开始拖。
const DRAG_START: f64 = 4.0;
/// 拖动时文件图标的大小（点）。
const DRAG_ICON: f64 = 48.0;

define_class!(
    // SAFETY: NSView 允许子类化；CapsuleView 没有实现 Drop。
    #[unsafe(super(NSView, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "ScreenFerryCapsuleView"]
    #[ivars = ViewIvars]
    struct CapsuleView;

    impl CapsuleView {
        /// 左上角为原点，与 Scene 一致。
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        /// 窗口不是当前窗口时，第一下点击也算数。
        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: CGRect) {
            draw(&self.ivars().scene.borrow(), self.ivars().scale.get());
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            let at = self.convertPoint_fromView(event.locationInWindow(), None);
            let scale = self.ivars().scale.get();
            let (x, y) = ((at.x / scale) as f32, (at.y / scale) as f32);
            let hit = self
                .ivars()
                .scene
                .borrow()
                .hits
                .iter()
                .find(|hit| hit.frame.contains(x, y))
                .copied();
            self.ivars().press.set(None);
            match hit {
                Some(hit) if hit.drag => {
                    if !self.ivars().offered.borrow().is_empty() {
                        self.ivars().press.set(Some(at));
                    }
                }
                Some(hit) => emit(OverlayEvent::Clicked(hit.id)),
                None => {}
            }
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            let Some(start) = self.ivars().press.get() else {
                return;
            };
            let at = self.convertPoint_fromView(event.locationInWindow(), None);
            if (at.x - start.x).hypot(at.y - start.y) >= DRAG_START {
                self.ivars().press.set(None);
                self.drag_out(event, start);
            }
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, _event: &NSEvent) {
            self.ivars().press.set(None);
        }
    }

    unsafe impl NSObjectProtocol for CapsuleView {}

    unsafe impl NSDraggingSource for CapsuleView {
        /// 拖到别的程序（访达、桌面、聊天窗口……）时由对方决定移动还是复制；拖回本程序不接。
        #[unsafe(method(draggingSession:sourceOperationMaskForDraggingContext:))]
        fn source_operation_mask(
            &self,
            _session: &NSDraggingSession,
            context: NSDraggingContext,
        ) -> NSDragOperation {
            if context == NSDraggingContext::OutsideApplication {
                NSDragOperation::Copy | NSDragOperation::Move | NSDragOperation::Generic
            } else {
                NSDragOperation::None
            }
        }

        #[unsafe(method(draggingSession:endedAtPoint:operation:))]
        fn dragging_ended(
            &self,
            _session: &NSDraggingSession,
            _at: CGPoint,
            operation: NSDragOperation,
        ) {
            emit(OverlayEvent::DragOutEnded {
                dropped: operation != NSDragOperation::None,
            });
        }
    }

    unsafe impl NSDraggingDestination for CapsuleView {
        #[unsafe(method(draggingEntered:))]
        fn dragging_entered(&self, sender: &ProtocolObject<dyn NSDraggingInfo>) -> NSDragOperation {
            // 从卡片拖出去的文件经过自己时不接。
            if sender.draggingSource().is_some() {
                return NSDragOperation::None;
            }
            let files = !file_urls(&sender.draggingPasteboard()).is_empty();
            emit(OverlayEvent::DragEntered { files });
            operation(files)
        }

        #[unsafe(method(draggingUpdated:))]
        fn dragging_updated(&self, sender: &ProtocolObject<dyn NSDraggingInfo>) -> NSDragOperation {
            if sender.draggingSource().is_some() {
                return NSDragOperation::None;
            }
            operation(!file_urls(&sender.draggingPasteboard()).is_empty())
        }

        #[unsafe(method(draggingExited:))]
        fn dragging_exited(&self, sender: Option<&ProtocolObject<dyn NSDraggingInfo>>) {
            if sender.is_some_and(|s| s.draggingSource().is_some()) {
                return;
            }
            emit(OverlayEvent::DragExited);
        }

        #[unsafe(method(prepareForDragOperation:))]
        fn prepare_for_drag(&self, _sender: &ProtocolObject<dyn NSDraggingInfo>) -> bool {
            true
        }

        #[unsafe(method(performDragOperation:))]
        fn perform_drag(&self, sender: &ProtocolObject<dyn NSDraggingInfo>) -> bool {
            let paths = file_urls(&sender.draggingPasteboard());
            let any = !paths.is_empty();
            if any {
                emit(OverlayEvent::Dropped { paths });
            }
            any
        }
    }
);

impl CapsuleView {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ViewIvars {
            scene: RefCell::new(Scene::default()),
            scale: Cell::new(1.0),
            offered: RefCell::new(Vec::new()),
            press: Cell::new(None),
        });
        let frame = CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(10.0, 10.0));
        // SAFETY: NSView 的指定初始化方法。
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    /// 把交来的文件拖出去：每个文件一项，带着它在访达里的图标，从按下的位置出发。
    fn drag_out(&self, event: &NSEvent, start: CGPoint) {
        let paths = self.ivars().offered.borrow().clone();
        if paths.is_empty() {
            return;
        }
        let workspace = NSWorkspace::sharedWorkspace();
        let items: Vec<Retained<NSDraggingItem>> = paths
            .iter()
            .enumerate()
            .map(|(i, path)| drag_item(&workspace, path, start, i))
            .collect();
        let items = NSArray::from_retained_slice(&items);
        let session = self.beginDraggingSessionWithItems_event_source(
            &items,
            event,
            ProtocolObject::from_ref(self),
        );
        session.setAnimatesToStartingPositionsOnCancelOrFail(true);
        emit(OverlayEvent::DragOutStarted);
    }
}

/// 拖动中的一项：文件的 URL 和图标。多个文件的图标稍微错开叠在一起。
fn drag_item(
    workspace: &NSWorkspace,
    path: &Path,
    start: CGPoint,
    index: usize,
) -> Retained<NSDraggingItem> {
    let string = NSString::from_str(&path.to_string_lossy());
    let url = NSURL::fileURLWithPath(&string);
    let item = NSDraggingItem::initWithPasteboardWriter(
        NSDraggingItem::alloc(),
        ProtocolObject::from_ref(&*url),
    );
    let icon = workspace.iconForFile(&string);
    icon.setSize(CGSize::new(DRAG_ICON, DRAG_ICON));
    let offset = index.min(4) as f64 * 6.0;
    let frame = CGRect::new(
        CGPoint::new(
            start.x - DRAG_ICON / 2.0 + offset,
            start.y - DRAG_ICON / 2.0 + offset,
        ),
        CGSize::new(DRAG_ICON, DRAG_ICON),
    );
    // SAFETY: 拖动项的内容可以是 NSImage。
    unsafe { item.setDraggingFrame_contents(frame, Some(&icon)) };
    item
}

/// 拖进来的是文件时复制，否则不接。
fn operation(files: bool) -> NSDragOperation {
    if files {
        NSDragOperation::Copy
    } else {
        NSDragOperation::None
    }
}

/// 剪贴板上的文件和文件夹路径。
fn file_urls(pasteboard: &NSPasteboard) -> Vec<PathBuf> {
    let classes: Retained<NSArray<AnyClass>> = NSArray::from_slice(&[NSURL::class()]);
    let yes = NSNumber::new_bool(true);
    // SAFETY: 读取 AppKit 导出的常量。
    let key = unsafe { NSPasteboardURLReadingFileURLsOnlyKey };
    let options = NSDictionary::from_slices(&[key], &[&*yes as &AnyObject]);
    // SAFETY: 类列表只含 NSURL，选项是 AppKit 规定的键。
    let Some(objects) =
        (unsafe { pasteboard.readObjectsForClasses_options(&classes, Some(&options)) })
    else {
        return Vec::new();
    };
    objects
        .iter()
        .filter_map(|object| object.downcast::<NSURL>().ok())
        .filter_map(|url| url.path())
        .map(|path| PathBuf::from(path.to_string()))
        .collect()
}

/// 主线程上的浮层窗口，第一次显示时创建。
struct Panel {
    panel: Retained<NSPanel>,
    view: Retained<CapsuleView>,
}

thread_local! {
    static PANEL: RefCell<Option<Panel>> = const { RefCell::new(None) };
}

fn create_panel(mtm: MainThreadMarker) -> Panel {
    let rect = CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(10.0, 10.0));
    let style = NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel;
    let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
        NSPanel::alloc(mtm),
        rect,
        style,
        NSBackingStoreType::Buffered,
        false,
    );
    // SAFETY: 窗口由本模块持有，关闭时不能被释放。
    unsafe { panel.setReleasedWhenClosed(false) };
    panel.setLevel(NSStatusWindowLevel);
    panel.setOpaque(false);
    panel.setBackgroundColor(Some(&NSColor::clearColor()));
    panel.setHasShadow(false);
    // 明确设为不忽略：整个窗口（包括透明的边）都接收拖放，文件推到屏幕边缘就能放下。
    panel.setIgnoresMouseEvents(false);
    panel.setHidesOnDeactivate(false);
    panel.setFloatingPanel(true);
    panel.setBecomesKeyOnlyIfNeeded(true);
    panel.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::Stationary
            | NSWindowCollectionBehavior::IgnoresCycle,
    );
    let view = CapsuleView::new(mtm);
    // SAFETY: 读取 AppKit 导出的常量。
    let file_url = unsafe { NSPasteboardTypeFileURL };
    view.registerForDraggedTypes(&NSArray::from_slice(&[file_url]));
    panel.setContentView(Some(&view));
    Panel { panel, view }
}

/// 在主线程上对浮层窗口做点事，没有就先创建。
fn on_panel(work: impl FnOnce(&Panel, MainThreadMarker) + Send + 'static) {
    DispatchQueue::main().exec_async(move || {
        let mtm = MainThreadMarker::new().expect("GCD 主队列运行在主线程上");
        PANEL.with(|cell| {
            let mut cell = cell.borrow_mut();
            let panel = cell.get_or_insert_with(|| create_panel(mtm));
            work(panel, mtm);
        });
    });
}

/// 桌面坐标（左上角原点）→ Cocoa 坐标（主屏幕左下角原点）。
fn cocoa_rect(frame: Frame, mtm: MainThreadMarker) -> CGRect {
    let height = NSScreen::screens(mtm)
        .firstObject()
        .map_or(0.0, |screen| screen.frame().size.height);
    CGRect::new(
        CGPoint::new(f64::from(frame.x), height - f64::from(frame.y + frame.h)),
        CGSize::new(f64::from(frame.w), f64::from(frame.h)),
    )
}

pub struct MacOverlay;

impl MacOverlay {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MacOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl Overlay for MacOverlay {
    fn start(&mut self, sink: OverlaySink) -> Result<()> {
        *SINK.lock().unwrap_or_else(|e| e.into_inner()) = Some(sink);
        Ok(())
    }

    fn show(&mut self, frame: Frame, scale: f32, scene: Scene) {
        on_panel(move |panel, mtm| {
            *panel.view.ivars().scene.borrow_mut() = scene;
            // macOS 的桌面坐标就是点，scale 恒为 1；仍按约定换算，保持与其他平台一致。
            panel.view.ivars().scale.set(f64::from(scale));
            panel.panel.setFrame_display(cocoa_rect(frame, mtm), false);
            panel.view.setNeedsDisplay(true);
            if !panel.panel.isVisible() {
                panel.panel.orderFrontRegardless();
            }
        });
    }

    fn hide(&mut self) {
        on_panel(|panel, _| panel.panel.orderOut(None));
    }

    fn drag_count(&self) -> i64 {
        // SAFETY: 读取 AppKit 导出的常量。
        let name = unsafe { NSPasteboardNameDrag };
        NSPasteboard::pasteboardWithName(name).changeCount() as i64
    }

    fn dragged_files(&self) -> Vec<PathBuf> {
        // SAFETY: 读取 AppKit 导出的常量。
        let name = unsafe { NSPasteboardNameDrag };
        file_urls(&NSPasteboard::pasteboardWithName(name))
    }

    fn offer(&mut self, files: Vec<PathBuf>) {
        on_panel(move |panel, _| *panel.view.ivars().offered.borrow_mut() = files);
    }

    fn open(&self, path: &Path) {
        let url = file_url(path);
        DispatchQueue::main().exec_async(move || {
            NSWorkspace::sharedWorkspace().openURL(&url.0);
        });
    }

    fn reveal(&self, path: &Path) {
        let url = file_url(path);
        DispatchQueue::main().exec_async(move || {
            let urls = NSArray::from_retained_slice(&[url.0]);
            NSWorkspace::sharedWorkspace().activateFileViewerSelectingURLs(&urls);
        });
    }
}

/// 可以交给主线程的文件 URL。
struct SendUrl(Retained<NSURL>);

// SAFETY: NSURL 是不可变对象，可以跨线程传递。
unsafe impl Send for SendUrl {}

fn file_url(path: &Path) -> SendUrl {
    SendUrl(NSURL::fileURLWithPath(&NSString::from_str(
        &path.to_string_lossy(),
    )))
}

/// 把一帧浮层画成 PNG，`scale` 为每点多少像素。
///
/// 与窗口里用的是同一套绘制代码，不需要窗口，也不需要屏幕录制权限，用于预览和核对样子。
/// 只能在主线程上调用。
pub fn render_png(scene: &Scene, scale: f64) -> Option<Vec<u8>> {
    let mtm = MainThreadMarker::new()?;
    let width = (f64::from(scene.width) * scale).ceil() as usize;
    let height = (f64::from(scene.height) * scale).ceil() as usize;
    let space = CGColorSpace::new_device_rgb()?;
    // SAFETY: 像素缓冲区由系统分配；参数是一份标准的 8 位 RGBA 位图。
    let context = unsafe {
        CGBitmapContextCreate(
            std::ptr::null_mut(),
            width,
            height,
            8,
            0,
            Some(&space),
            CGImageAlphaInfo::PremultipliedLast.0,
        )
    }?;
    // 位图原点在左下角，翻过来与视图一致。
    CGContext::translate_ctm(Some(&context), 0.0, height as f64);
    CGContext::scale_ctm(Some(&context), 1.0, -1.0);
    let graphics = NSGraphicsContext::graphicsContextWithCGContext_flipped(&context, true);
    NSGraphicsContext::saveGraphicsState_class();
    NSGraphicsContext::setCurrentContext(Some(&graphics));
    draw(scene, scale);
    NSGraphicsContext::restoreGraphicsState_class();
    let image = CGBitmapContextCreateImage(Some(&context))?;
    let rep = NSBitmapImageRep::initWithCGImage(NSBitmapImageRep::alloc(), &image);
    let _ = mtm;
    // SAFETY: 不带任何属性导出 PNG。
    let data = unsafe {
        rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }?;
    Some(data.to_vec())
}

// ── 绘制 ─────────────────────────────────────

fn draw(scene: &Scene, scale: f64) {
    for shape in &scene.shapes {
        match shape {
            Shape::RoundRect {
                frame,
                radius,
                fill,
            } => {
                color(*fill).setFill();
                round_rect(*frame, *radius, scale).fill();
            }
            Shape::RoundRectStroke {
                frame,
                radius,
                width,
                color: stroke,
                dash,
            } => {
                let path = round_rect(*frame, *radius, scale);
                path.setLineWidth(f64::from(*width) * scale);
                if *dash > 0.0 {
                    let pattern = [f64::from(*dash) * scale; 2];
                    // SAFETY: 指针指向两个有效的长度值，调用期间一直有效。
                    unsafe { path.setLineDash_count_phase(pattern.as_ptr(), 2, 0.0) };
                }
                color(*stroke).setStroke();
                path.stroke();
            }
            Shape::Circle { cx, cy, r, fill } => {
                let rect = rect(Frame::new(cx - r, cy - r, 2.0 * r, 2.0 * r), scale);
                color(*fill).setFill();
                NSBezierPath::bezierPathWithOvalInRect(rect).fill();
            }
            Shape::Arc {
                cx,
                cy,
                r,
                width,
                sweep,
                color: stroke,
            } => {
                if *sweep <= 0.0 {
                    continue;
                }
                let path = NSBezierPath::bezierPath();
                let center = CGPoint::new(f64::from(*cx) * scale, f64::from(*cy) * scale);
                // 视图是翻转的：角度增大即屏幕上的顺时针。从 12 点钟方向（-90°）开始。
                let end = -90.0 + 360.0 * f64::from(sweep.min(0.9999));
                path.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle_clockwise(
                    center,
                    f64::from(*r) * scale,
                    -90.0,
                    end,
                    false,
                );
                path.setLineWidth(f64::from(*width) * scale);
                path.setLineCapStyle(NSLineCapStyle::Round);
                color(*stroke).setStroke();
                path.stroke();
            }
            Shape::Polyline {
                points,
                width,
                color: stroke,
            } => {
                let Some((first, rest)) = points.split_first() else {
                    continue;
                };
                let path = NSBezierPath::bezierPath();
                path.moveToPoint(point(*first, scale));
                for p in rest {
                    path.lineToPoint(point(*p, scale));
                }
                path.setLineWidth(f64::from(*width) * scale);
                path.setLineCapStyle(NSLineCapStyle::Round);
                path.setLineJoinStyle(NSLineJoinStyle::Round);
                color(*stroke).setStroke();
                path.stroke();
            }
            Shape::Text {
                frame,
                text,
                size,
                weight,
                color: ink,
                align,
            } => draw_text(*frame, text, *size, *weight, *ink, *align, scale),
        }
    }
}

fn draw_text(
    frame: Frame,
    text: &str,
    size: f32,
    weight: Weight,
    ink: Rgba,
    align: Align,
    scale: f64,
) {
    // SAFETY: 读取 AppKit 导出的常量。
    let (weight, font_key, color_key, style_key) = unsafe {
        (
            match weight {
                Weight::Regular => NSFontWeightRegular,
                Weight::Semibold => NSFontWeightSemibold,
            },
            NSFontAttributeName,
            NSForegroundColorAttributeName,
            NSParagraphStyleAttributeName,
        )
    };
    let font = NSFont::systemFontOfSize_weight(f64::from(size) * scale, weight);
    let style = NSMutableParagraphStyle::new();
    style.setAlignment(match align {
        Align::Left => NSTextAlignment::Left,
        Align::Center => NSTextAlignment::Center,
    });
    style.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    let ink = color(ink);
    let values: [&AnyObject; 3] = [&font, &ink, &style];
    let attrs = NSDictionary::from_slices(&[font_key, color_key, style_key], &values);
    let string = NSString::from_str(text);
    // SAFETY: 属性字典只含字体、颜色和段落样式这三个 AppKit 认识的键。
    let height = unsafe { string.sizeWithAttributes(Some(&attrs)) }.height;
    let box_ = rect(frame, scale);
    let line = CGRect::new(
        CGPoint::new(
            box_.origin.x,
            box_.origin.y + (box_.size.height - height) / 2.0,
        ),
        CGSize::new(box_.size.width, height),
    );
    // SAFETY: 同上。
    unsafe { string.drawInRect_withAttributes(line, Some(&attrs)) };
}

fn round_rect(frame: Frame, radius: f32, scale: f64) -> Retained<NSBezierPath> {
    let r = f64::from(radius.min(frame.w / 2.0).min(frame.h / 2.0).max(0.0)) * scale;
    NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect(frame, scale), r, r)
}

fn rect(frame: Frame, scale: f64) -> CGRect {
    CGRect::new(
        CGPoint::new(f64::from(frame.x) * scale, f64::from(frame.y) * scale),
        CGSize::new(f64::from(frame.w) * scale, f64::from(frame.h) * scale),
    )
}

fn point((x, y): (f32, f32), scale: f64) -> CGPoint {
    CGPoint::new(f64::from(x) * scale, f64::from(y) * scale)
}

fn color(c: Rgba) -> Retained<NSColor> {
    let k = |v: u8| CGFloat::from(v) / 255.0;
    NSColor::colorWithSRGBRed_green_blue_alpha(k(c.r), k(c.g), k(c.b), k(c.a))
}
