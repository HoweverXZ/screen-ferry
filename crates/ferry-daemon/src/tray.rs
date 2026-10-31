//! 托盘图标：显示共享状态，菜单里可以暂停或继续共享、打开设置、退出。
//!
//! 托盘对象只能在主线程上创建和修改，所以放在主线程的线程局部变量里；
//! 状态变化时由引擎所在的线程经 [`Dispatcher`] 把更新交给主线程。

use std::cell::RefCell;

use ferry_engine::resident::{Command, Handle, Summary};
use ferry_platform::Dispatcher;
use tokio::sync::watch;
use tracing::warn;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon};

const TOGGLE: &str = "toggle";
const OPEN: &str = "open";
const QUIT: &str = "quit";

/// 菜单栏用模板图：只看透明度，系统按明暗模式自动着色。
#[cfg(target_os = "macos")]
mod art {
    use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

    pub(super) const ACTIVE: &[u8] = include_bytes!("../../../assets/icons/tray-template@2x.png");
    pub(super) const PAUSED: &[u8] = include_bytes!("../../../assets/icons/tray-paused@2x.png");

    pub(super) fn builder(icon: Icon) -> TrayIconBuilder {
        TrayIconBuilder::new().with_icon_templated(icon)
    }

    pub(super) fn set(tray: &TrayIcon, icon: Icon) -> tray_icon::Result<()> {
        tray.set_icon_templated(Some(icon))
    }
}

/// 任务栏可能是深色也可能是浅色，用带底色的彩色图标。
#[cfg(target_os = "windows")]
mod art {
    use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

    pub(super) const ACTIVE: &[u8] = include_bytes!("../../../assets/icons/tray-color.png");
    pub(super) const PAUSED: &[u8] = include_bytes!("../../../assets/icons/tray-color-paused.png");

    pub(super) fn builder(icon: Icon) -> TrayIconBuilder {
        TrayIconBuilder::new().with_icon(icon)
    }

    pub(super) fn set(tray: &TrayIcon, icon: Icon) -> tray_icon::Result<()> {
        tray.set_icon(Some(icon))
    }
}

struct Tray {
    icon: TrayIcon,
    status: MenuItem,
    toggle: MenuItem,
    active: Icon,
    paused: Icon,
    shown: Option<Summary>,
}

thread_local! {
    static TRAY: RefCell<Option<Tray>> = const { RefCell::new(None) };
}

/// 在主线程上创建托盘图标，菜单命令转给 `handle`。
pub(crate) fn install(handle: Handle) -> Result<(), Box<dyn std::error::Error>> {
    let status = MenuItem::new("正在启动…", false, None);
    let toggle = MenuItem::with_id(TOGGLE, "暂停共享", true, None);
    let open = MenuItem::with_id(OPEN, "打开设置…", true, None);
    let quit = MenuItem::with_id(QUIT, "退出 ScreenFerry", true, None);
    let menu = Menu::new();
    menu.append_items(&[
        &status,
        &PredefinedMenuItem::separator(),
        &toggle,
        &open,
        &PredefinedMenuItem::separator(),
        &quit,
    ])?;

    let active = decode(art::ACTIVE)?;
    let paused = decode(art::PAUSED)?;
    let icon = art::builder(active.clone())
        .with_tooltip("ScreenFerry")
        .with_menu(Box::new(menu))
        .build()?;

    MenuEvent::set_event_handler(Some(move |event: MenuEvent| match event.id.0.as_str() {
        TOGGLE => {
            let sharing = handle.summary().borrow().sharing;
            handle.send(Command::SetSharing(!sharing));
        }
        OPEN => handle.send(Command::OpenSettings),
        QUIT => handle.send(Command::Quit),
        _ => {}
    }));

    TRAY.with(|tray| {
        *tray.borrow_mut() = Some(Tray {
            icon,
            status,
            toggle,
            active,
            paused,
            shown: None,
        });
    });
    Ok(())
}

/// 状态一变就请主线程更新托盘。在引擎所在的 tokio 运行时里运行。
pub(crate) async fn follow(mut summary: watch::Receiver<Summary>, dispatcher: Dispatcher) {
    loop {
        let current = summary.borrow_and_update().clone();
        dispatcher.run_on_main(move || update(&current));
        if summary.changed().await.is_err() {
            return;
        }
    }
}

/// 在主线程上调用。
fn update(summary: &Summary) {
    TRAY.with(|tray| {
        let mut tray = tray.borrow_mut();
        let Some(tray) = tray.as_mut() else { return };
        if tray.shown.as_ref() == Some(summary) {
            return;
        }
        let status = describe(summary);
        tray.status.set_text(&status);
        tray.toggle.set_text(if summary.sharing {
            "暂停共享"
        } else {
            "继续共享"
        });
        let icon = if summary.sharing && summary.problem.is_none() {
            &tray.active
        } else {
            &tray.paused
        };
        let set = art::set(&tray.icon, icon.clone());
        if let Err(e) = set {
            warn!("更新托盘图标失败：{e}");
        }
        let _ = tray
            .icon
            .set_tooltip(Some(format!("ScreenFerry · {status}")));
        tray.shown = Some(summary.clone());
    });
}

/// 菜单第一行的状态文字。
fn describe(summary: &Summary) -> String {
    if let Some(problem) = &summary.problem {
        let short: String = problem.chars().take(28).collect();
        let ellipsis = if problem.chars().count() > 28 {
            "…"
        } else {
            ""
        };
        return format!("⚠︎ {short}{ellipsis}");
    }
    match (&summary.connected_to, summary.sharing) {
        (_, false) => "已暂停共享".to_owned(),
        (Some(peer), true) => format!("已连接 {peer}"),
        (None, true) => "共享中 · 等待连接".to_owned(),
    }
}

fn decode(png_bytes: &[u8]) -> Result<Icon, Box<dyn std::error::Error>> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA);
    let mut reader = decoder.read_info()?;
    let mut rgba = vec![0; reader.output_buffer_size().ok_or("图标太大")?];
    let info = reader.next_frame(&mut rgba)?;
    rgba.truncate(info.buffer_size());
    Ok(Icon::from_rgba(rgba, info.width, info.height)?)
}
