use ferry_platform::CaptureEvent;
use ferry_types::{InputEvent, KeyAction, KeyCode, MouseButton, Point};

use crate::support::{EDGE, ENTRY, Rig, eventually};

fn motion(x: i32, y: i32) -> InputEvent {
    InputEvent::PointerMotion {
        position: Point::new(x, y),
    }
}

#[tokio::test]
async fn cursor_crosses_to_the_client_and_back() {
    let rig = Rig::start().await;
    rig.enter().await;

    rig.push(10, 0); // 150% 缩放：10 → 15
    rig.capture.emit(CaptureEvent::Button {
        button: MouseButton::LEFT,
        pressed: true,
    });
    rig.capture.emit(CaptureEvent::Button {
        button: MouseButton::LEFT,
        pressed: false,
    });
    rig.push(-20, 0); // 15 - 30 < 0：从左边缘回到本机

    eventually("回到本机", || !rig.capture.is_grabbed()).await;
    assert_eq!(rig.capture.last_release(), Some(EDGE), "纵坐标换算回 400");

    let at = Point::new(15, 540);
    eventually("客户端收到全部事件", || {
        rig.inject.events().len() >= 4
    })
    .await;
    assert_eq!(
        rig.inject.events(),
        [
            motion(ENTRY.x, ENTRY.y),
            motion(at.x, at.y),
            InputEvent::Button {
                button: MouseButton::LEFT,
                pressed: true,
                position: at
            },
            InputEvent::Button {
                button: MouseButton::LEFT,
                pressed: false,
                position: at
            },
        ],
    );
}

#[tokio::test]
async fn holding_a_button_keeps_the_cursor_on_the_client() {
    let rig = Rig::start().await;
    rig.enter().await;

    rig.capture.emit(CaptureEvent::Button {
        button: MouseButton::LEFT,
        pressed: true,
    });
    rig.push(-500, 0);
    eventually("客户端收到按下", || rig.inject.events().len() >= 2).await;
    assert!(rig.capture.is_grabbed(), "拖拽中不能切回本机");

    rig.capture.emit(CaptureEvent::Button {
        button: MouseButton::LEFT,
        pressed: false,
    });
    rig.push(-1, 0);
    eventually("松开后可以回到本机", || !rig.capture.is_grabbed()).await;
}

#[tokio::test]
async fn control_returns_when_the_client_disconnects() {
    let rig = Rig::start().await;
    rig.enter().await;

    rig.client.abort();
    eventually("客户端断开后交还控制", || {
        !rig.capture.is_grabbed()
    })
    .await;
    assert_eq!(rig.capture.last_release(), Some(EDGE));
}

fn key(code: KeyCode, action: KeyAction) -> InputEvent {
    InputEvent::Key { code, action }
}

#[tokio::test]
async fn keys_are_forwarded_with_command_as_control() {
    let rig = Rig::start().await;
    let letter_c = KeyCode::keyboard(0x06);

    rig.key(letter_c, KeyAction::Down);
    rig.key(letter_c, KeyAction::Up);
    rig.enter().await;
    rig.key(KeyCode::LEFT_GUI, KeyAction::Down);
    rig.key(letter_c, KeyAction::Down);
    rig.key(letter_c, KeyAction::Repeat);
    rig.key(letter_c, KeyAction::Up);
    rig.key(KeyCode::LEFT_GUI, KeyAction::Up);

    eventually("客户端收到按键", || rig.inject.events().len() >= 6).await;
    assert_eq!(
        rig.inject.events(),
        [
            motion(ENTRY.x, ENTRY.y),
            key(KeyCode::LEFT_CTRL, KeyAction::Down),
            key(letter_c, KeyAction::Down),
            key(letter_c, KeyAction::Repeat),
            key(letter_c, KeyAction::Up),
            key(KeyCode::LEFT_CTRL, KeyAction::Up),
        ],
        "光标进入前的按键不转发；⌘ 换成 Ctrl",
    );
}

#[tokio::test]
async fn media_keys_are_forwarded_and_released_on_leave() {
    let rig = Rig::start().await;
    rig.enter().await;
    rig.key(KeyCode::VOLUME_UP, KeyAction::Down);
    rig.key(KeyCode::VOLUME_UP, KeyAction::Repeat);
    rig.key(KeyCode::VOLUME_UP, KeyAction::Up);
    rig.key(KeyCode::PLAY_PAUSE, KeyAction::Down);
    rig.leave().await; // 播放键还按着就离开了

    eventually("客户端收到媒体键并松开", || {
        rig.inject.events().len() >= 6
    })
    .await;
    let keys: Vec<_> = rig
        .inject
        .events()
        .into_iter()
        .filter(|e| matches!(e, InputEvent::Key { .. }))
        .collect();
    assert_eq!(
        keys,
        [
            key(KeyCode::VOLUME_UP, KeyAction::Down),
            key(KeyCode::VOLUME_UP, KeyAction::Repeat),
            key(KeyCode::VOLUME_UP, KeyAction::Up),
            key(KeyCode::PLAY_PAUSE, KeyAction::Down),
            key(KeyCode::PLAY_PAUSE, KeyAction::Up),
        ],
        "媒体键原样转发，离开时松开",
    );
}

#[tokio::test]
async fn ctrl_option_escape_returns_to_local() {
    let rig = Rig::start().await;
    rig.enter().await;

    rig.key(KeyCode::LEFT_CTRL, KeyAction::Down);
    rig.key(KeyCode::RIGHT_ALT, KeyAction::Down);
    rig.key(KeyCode::ESCAPE, KeyAction::Down);

    eventually("快捷键回到本机", || !rig.capture.is_grabbed()).await;
    assert_eq!(rig.capture.last_release(), Some(EDGE));

    // 客户端收到 Leave 后松开按着的修饰键（Ctrl 已被映射成 Win），Esc 本身不转发。
    eventually("客户端松开修饰键", || {
        rig.inject.events().len() >= 5
    })
    .await;
    assert_eq!(
        rig.inject.events()[1..],
        [
            key(KeyCode::LEFT_GUI, KeyAction::Down),
            key(KeyCode::RIGHT_ALT, KeyAction::Down),
            key(KeyCode::LEFT_GUI, KeyAction::Up),
            key(KeyCode::RIGHT_ALT, KeyAction::Up),
        ],
    );
}

#[tokio::test]
async fn changing_side_mid_session_takes_effect_without_reconnecting() {
    use ferry_core::{KeyRemap, Side};
    use ferry_engine::Live;
    use tokio::sync::watch;

    use crate::support::Setup;

    let (tx, rx) = watch::channel(Live {
        side: Side::Right,
        remap: KeyRemap::none(),
    });
    let rig = Rig::with(Setup {
        live: Some(rx),
        ..Setup::default()
    })
    .await;
    rig.enter().await;

    // 光标在对方屏幕时改方向：先回到本机。
    tx.send_modify(|live| live.side = Side::Left);
    eventually("改方向后回到本机", || !rig.capture.is_grabbed()).await;

    // 右边缘不再切换，左边缘可以。
    for _ in 0..5 {
        rig.push(5, 0);
    }
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert!(!rig.capture.is_grabbed(), "右边缘已经不通了");
    eventually("从左边缘进入", || {
        if rig.capture.is_grabbed() {
            return true;
        }
        rig.capture.emit(CaptureEvent::Pointer {
            position: Point::new(0, 400),
            dx: -5,
            dy: 0,
        });
        false
    })
    .await;
}
