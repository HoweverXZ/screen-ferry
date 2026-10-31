use ferry_platform::CaptureEvent;
use ferry_types::{InputEvent, LockKeys, MouseButton};

use crate::support::{ENTRY, Rig, eventually};

#[tokio::test]
async fn buttons_are_released_when_the_server_disappears() {
    let rig = Rig::start().await;
    rig.enter().await;
    rig.capture.emit(CaptureEvent::Button {
        button: MouseButton::LEFT,
        pressed: true,
    });
    eventually("客户端收到按下", || rig.inject.events().len() >= 2).await;

    rig.server.abort();

    let released = InputEvent::Button {
        button: MouseButton::LEFT,
        pressed: false,
        position: ENTRY,
    };
    eventually("服务端消失后松开鼠标键", || {
        rig.inject.events().last() == Some(&released)
    })
    .await;
}

#[tokio::test]
async fn entering_syncs_caps_lock() {
    let rig = Rig::start().await;
    rig.capture.set_lock_keys(LockKeys::CAPS | LockKeys::NUM);
    rig.enter().await;

    eventually("客户端同步锁定键", || {
        !rig.inject.synced_locks().is_empty()
    })
    .await;
    assert_eq!(
        rig.inject.synced_locks(),
        [LockKeys::CAPS],
        "只同步 Caps Lock"
    );
}

#[tokio::test]
async fn keys_are_released_when_the_client_engine_is_stopped() {
    use ferry_types::{KeyAction, KeyCode};

    let rig = Rig::start().await;
    rig.enter().await;
    rig.key(KeyCode::LEFT_SHIFT, KeyAction::Down);
    let down = InputEvent::Key {
        code: KeyCode::LEFT_SHIFT,
        action: KeyAction::Down,
    };
    eventually("客户端按下 Shift", || {
        rig.inject.events().contains(&down)
    })
    .await;

    // 常驻后台暂停共享、重启引擎时，会直接丢掉客户端的任务。
    rig.client.abort();

    let up = InputEvent::Key {
        code: KeyCode::LEFT_SHIFT,
        action: KeyAction::Up,
    };
    eventually("引擎停下时松开 Shift", || {
        rig.inject.events().contains(&up)
    })
    .await;
}
