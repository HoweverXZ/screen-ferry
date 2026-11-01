use std::fs;
use std::sync::Arc;
use std::time::Duration;

use ferry_core::capsule::hit;
use ferry_engine::{EdgeDrop, Event, Events, Files};
use ferry_platform::{CaptureEvent, OverlayEvent};
use ferry_platform_mock::MockOverlay;
use ferry_types::{MouseButton, Point, Rgba, Shape};
use tokio::time::sleep;

use crate::support::{Device, Rig, Setup, TempDir, eventually};

const ACCENT: Rgba = Rgba::hex(0x0A6CFF, 255);

/// 一台带着边缘投递的服务端（本机 1000×800，客户端在右侧），会话已经建好、光标在本机。
///
/// 常驻后台会把文件传输的进展转给边缘投递，这里由一个小任务代劳。
async fn rig_with_edge_drop() -> (Rig, MockOverlay, Arc<EdgeDrop>) {
    let mac = Device::new();
    let (events, mut rx) = Events::channel();
    let files = Files::new(&mac.store, events);
    let overlay = MockOverlay::new();
    let edge = Arc::new(EdgeDrop::spawn(overlay.clone(), files.clone(), ACCENT));
    tokio::spawn({
        let edge = edge.clone();
        async move {
            while let Some(event) = rx.recv().await {
                if let Event::Transfer(update) = event {
                    edge.transfer(update);
                }
            }
        }
    });
    let rig = Rig::with(Setup {
        mac,
        mac_files: Some(files),
        edge: edge.feed(),
        ..Setup::default()
    })
    .await;
    // 先切过去再回来：确认会话已经建好，光标回到本机。
    rig.enter().await;
    rig.leave().await;
    (rig, overlay, edge)
}

fn texts(overlay: &MockOverlay) -> Vec<String> {
    overlay.shown().map_or_else(Vec::new, |(_, scene)| {
        scene
            .shapes
            .into_iter()
            .filter_map(|s| match s {
                Shape::Text { text, .. } => Some(text),
                _ => None,
            })
            .collect()
    })
}

fn pointer(rig: &Rig, x: i32, y: i32) {
    rig.capture.emit(CaptureEvent::Pointer {
        position: Point::new(x, y),
        dx: 1,
        dy: 0,
    });
}

fn left_button(rig: &Rig, pressed: bool) {
    rig.capture.emit(CaptureEvent::Button {
        button: MouseButton::LEFT,
        pressed,
    });
}

/// 按下鼠标，再开始拖 `files`。真机上系统要等鼠标移动几个像素才开始拖，按下总是先被看到；
/// 这里等一下，模拟同样的先后。
async fn press_and_drag(rig: &Rig, overlay: &MockOverlay, files: Option<Vec<std::path::PathBuf>>) {
    left_button(rig, true);
    sleep(Duration::from_millis(50)).await;
    if let Some(files) = files {
        overlay.start_drag(files);
    }
}

#[tokio::test]
async fn dragging_files_to_the_edge_shows_the_capsule_and_dropping_sends_them() {
    let (rig, overlay, _edge) = rig_with_edge_drop().await;
    let source = TempDir::new();
    let file = source.path().join("证书.p12");
    fs::create_dir_all(source.path()).unwrap();
    fs::write(&file, b"secret").unwrap();

    // 按下鼠标，开始拖文件，再拖向右边缘（对方在右侧）。
    pointer(&rig, 500, 400);
    press_and_drag(&rig, &overlay, Some(vec![file.clone()])).await;
    eventually("靠近边缘时滑出胶囊", || {
        pointer(&rig, 900, 400);
        overlay.shown().is_some()
    })
    .await;
    let (window, _) = overlay.shown().unwrap();
    assert!(window.right() > 900.0, "贴着右边缘：{window:?}");

    // 拖到胶囊上：展开成“松手发送”。
    overlay.emit(OverlayEvent::DragEntered { files: true });
    eventually("展开成松手发送", || {
        texts(&overlay).contains(&"松手发送到".to_owned())
    })
    .await;
    assert!(texts(&overlay).contains(&"证书.p12".to_owned()));

    overlay.emit(OverlayEvent::Dropped {
        paths: vec![file.clone()],
    });
    left_button(&rig, false);
    let got = rig.pc.store.downloads_dir().join("证书.p12");
    eventually("对方收到文件", || got.is_file()).await;
    assert_eq!(fs::read(&got).unwrap(), b"secret");
    eventually("胶囊显示已发送", || {
        texts(&overlay).contains(&"已发送".to_owned())
    })
    .await;
}

#[tokio::test]
async fn a_received_file_shows_a_card_that_opens_it() {
    let (rig, overlay, _edge) = rig_with_edge_drop().await;
    let source = TempDir::new();
    let file = source.path().join("会议纪要.docx");
    fs::create_dir_all(source.path()).unwrap();
    fs::write(&file, b"notes").unwrap();

    // 等会话建好、边缘报告给了边缘投递，再从客户端发过来。
    let mut sent = false;
    for _ in 0..300 {
        if rig.pc_files.send(vec![file.clone()]).await.is_ok() {
            sent = true;
            break;
        }
        sleep(Duration::from_millis(10)).await;
    }
    assert!(sent, "客户端发出了文件");

    eventually("收完变成卡片", || {
        texts(&overlay).contains(&"打开".to_owned())
    })
    .await;
    assert!(texts(&overlay).contains(&"会议纪要.docx".to_owned()));
    // 点“打开”之前卡片要先完全出来，按钮才能点。
    sleep(Duration::from_millis(600)).await;
    let (_, scene) = overlay.shown().unwrap();
    assert!(scene.hits.iter().any(|h| h.id == hit::OPEN));

    overlay.emit(OverlayEvent::Clicked(hit::OPEN));
    let expected = rig.mac.store.downloads_dir().join("会议纪要.docx");
    eventually("用默认程序打开收到的文件", || {
        overlay.opened() == vec![expected.clone()]
    })
    .await;
    eventually("打开后卡片收起", || overlay.shown().is_none()).await;
}

#[tokio::test]
async fn dragging_something_other_than_files_shows_nothing() {
    let (rig, overlay, _edge) = rig_with_edge_drop().await;
    // 先确认拖文件时会出现胶囊，后面“没出现”才有意义。
    let source = TempDir::new();
    fs::create_dir_all(source.path()).unwrap();
    let file = source.path().join("a.txt");
    fs::write(&file, b"a").unwrap();
    press_and_drag(&rig, &overlay, Some(vec![file])).await;
    eventually("拖文件时出现胶囊", || {
        pointer(&rig, 900, 400);
        overlay.shown().is_some()
    })
    .await;
    left_button(&rig, false);
    eventually("松开后收起", || overlay.shown().is_none()).await;

    // 拖的是文字：拖放剪贴板变了，但里面没有文件。
    press_and_drag(&rig, &overlay, Some(Vec::new())).await;
    for x in [700, 850, 950, 999] {
        pointer(&rig, x, 400);
        sleep(Duration::from_millis(30)).await;
    }
    assert!(overlay.shown().is_none(), "拖文字不出现胶囊");
    left_button(&rig, false);

    // 拖窗口：拖放剪贴板根本没变。
    press_and_drag(&rig, &overlay, None).await;
    for x in [700, 850, 950, 999] {
        pointer(&rig, x, 400);
        sleep(Duration::from_millis(30)).await;
    }
    assert!(overlay.shown().is_none(), "拖窗口不出现胶囊");
    left_button(&rig, false);
}

#[tokio::test]
async fn without_a_drag_clipboard_the_edge_is_watched_until_files_come_in() {
    let (rig, overlay, _edge) = rig_with_edge_drop().await;
    let source = TempDir::new();
    fs::create_dir_all(source.path()).unwrap();
    let file = source.path().join("素材.zip");
    fs::write(&file, b"zip").unwrap();

    // 像 Windows 那样：按下鼠标靠近边缘时，还不知道拖的是什么。
    press_and_drag(&rig, &overlay, None).await;
    eventually("在贴边留意", || {
        pointer(&rig, 960, 400);
        overlay.watching().is_some()
    })
    .await;
    let strip = overlay.watching().unwrap();
    assert!(
        strip.x >= 990.0 && strip.right() <= 1000.5,
        "贴着右边缘的一窄条：{strip:?}"
    );
    assert!(overlay.shown().is_none(), "还不知道是文件时不出现胶囊");

    // 拖着文件进了接收条：直接展开成“松手发送”，不再留意。
    overlay.start_drag(vec![file]);
    overlay.emit(OverlayEvent::DragEntered { files: true });
    eventually("展开成松手发送", || {
        texts(&overlay).contains(&"松手发送到".to_owned())
    })
    .await;
    eventually("不再留意", || overlay.watching().is_none()).await;
    left_button(&rig, false);
}

/// 一台带着边缘投递的客户端（1920×1080，服务端在它的左侧），会话已经建好。
async fn client_with_edge_drop() -> (Rig, MockOverlay, Arc<EdgeDrop>) {
    let pc = Device::new();
    let (events, mut rx) = Events::channel();
    let files = Files::new(&pc.store, events);
    let overlay = MockOverlay::new();
    let edge = Arc::new(EdgeDrop::spawn(overlay.clone(), files.clone(), ACCENT));
    tokio::spawn({
        let edge = edge.clone();
        async move {
            while let Some(event) = rx.recv().await {
                if let Event::Transfer(update) = event {
                    edge.transfer(update);
                }
            }
        }
    });
    let rig = Rig::with(Setup {
        pc,
        pc_files: Some(files),
        pc_edge: edge.feed(),
        ..Setup::default()
    })
    .await;
    rig.enter().await;
    rig.leave().await;
    (rig, overlay, edge)
}

#[tokio::test]
async fn the_client_learns_its_edge_from_the_server_and_shows_received_files() {
    let (rig, overlay, _edge) = client_with_edge_drop().await;
    let source = TempDir::new();
    fs::create_dir_all(source.path()).unwrap();
    let file = source.path().join("季度报告.pdf");
    fs::write(&file, b"pdf").unwrap();

    rig.mac_files
        .send(vec![file])
        .await
        .expect("服务端发出文件");
    eventually("客户端收完变成卡片", || {
        texts(&overlay).contains(&"打开".to_owned())
    })
    .await;
    let (window, _) = overlay.shown().unwrap();
    assert!(
        window.x < 100.0,
        "服务端在客户端左侧，卡片贴左边缘：{window:?}"
    );
}

#[tokio::test]
async fn dragging_files_to_the_edge_on_the_client_sends_them_to_the_server() {
    let (rig, overlay, _edge) = client_with_edge_drop().await;
    let source = TempDir::new();
    fs::create_dir_all(source.path()).unwrap();
    let file = source.path().join("会议纪要.docx");
    fs::write(&file, b"notes").unwrap();

    // 光标进入客户端（从它的左边缘进来），按下鼠标：服务端把按键转给客户端注入。
    rig.enter().await;
    left_button(&rig, true);
    eventually("客户端在贴边留意", || {
        rig.push(-1, 0);
        overlay.watching().is_some()
    })
    .await;

    // 拖着文件推到左边缘，进了接收条。
    overlay.start_drag(vec![file.clone()]);
    overlay.emit(OverlayEvent::DragEntered { files: true });
    eventually("客户端展开成松手发送", || {
        texts(&overlay).contains(&"松手发送到".to_owned())
    })
    .await;
    overlay.emit(OverlayEvent::Dropped { paths: vec![file] });
    left_button(&rig, false);

    let got = rig.mac.store.downloads_dir().join("会议纪要.docx");
    eventually("服务端收到文件", || got.is_file()).await;
    assert_eq!(fs::read(&got).unwrap(), b"notes");
}
