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

#[tokio::test]
async fn the_local_cursor_is_hidden_while_the_pointer_is_on_the_server() {
    let rig = Rig::start().await;
    eventually(
        "连上后隐藏本机光标（光标在服务端）",
        || rig.inject.cursor_hidden() == [true],
    )
    .await;

    rig.enter().await;
    eventually("光标过来时显示", || {
        rig.inject.cursor_hidden() == [true, false]
    })
    .await;

    rig.leave().await;
    eventually("光标离开时再隐藏", || {
        rig.inject.cursor_hidden() == [true, false, true]
    })
    .await;

    // 服务端消失：会话结束，光标一定要恢复显示。
    rig.server.abort();
    eventually("断开后恢复显示", || {
        rig.inject.cursor_hidden() == [true, false, true, false]
    })
    .await;
}

/// 显示本机光标没做成时，注入的输入顺手再试一次：失败只记日志的话，光标会一直藏着，
/// 用户能操作却找不到指针。
#[tokio::test]
async fn a_failed_cursor_change_is_retried_on_the_next_input() {
    let rig = Rig::start().await;
    eventually("连上后隐藏本机光标", || rig.inject.cursor_hidden() == [true]).await;

    // 平台那边这一次没做成：进入时要求显示，失败。
    rig.inject.fail_cursor(1);
    rig.enter().await;
    // 接着注入的输入把它补上。没有重试的话这里只有 [true, false]。
    rig.push_at(500, 540, 10, 0);
    eventually(
        "下一次注入输入时重新显示",
        || rig.inject.cursor_hidden() == [true, false, false],
    )
    .await;
}

// ── 按地址连接 ───────────────────────────────

mod by_address {
    use ferry_engine::{ClientCommand, DialError, Event};

    use crate::support::{Rig, Setup};

    async fn server_only() -> Rig {
        Rig::with(Setup {
            start_client: false,
            ..Setup::default()
        })
        .await
    }

    #[tokio::test]
    async fn a_client_connects_to_an_address_when_asked_and_pairs() {
        let rig = server_only().await;
        let mut pc = rig.idle_client("书房", false);
        pc.commands.send(ClientCommand::Connect(rig.addr)).unwrap();

        let error = pc
            .wait_for("按地址连接的结果", |e| match e {
                Event::Dialed { error, invite, .. } if !invite => Some(*error),
                _ => None,
            })
            .await;
        assert_eq!(error, None, "连上了，开始核对配对码");
        // 配对完立即用这个地址重连，不用等局域网发现。
        pc.wait_for("配对后连上服务端", |e| {
            matches!(e, Event::Connected { .. }).then_some(())
        })
        .await;
        let peers = pc.device.store.peers().unwrap();
        let server = peers.iter().next().expect("记住了服务端");
        assert_eq!(server.addr, Some(rig.addr), "记下了服务端的地址");
    }

    #[tokio::test]
    async fn an_address_where_nothing_waits_is_reported() {
        let rig = server_only().await;
        let closed = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            listener.local_addr().unwrap()
        };
        let mut pc = rig.idle_client("书房", false);
        pc.commands.send(ClientCommand::Connect(closed)).unwrap();
        let error = pc
            .wait_for("按地址连接的结果", |e| match e {
                Event::Dialed { error, .. } => Some(*error),
                _ => None,
            })
            .await;
        assert_eq!(error, Some(DialError::Refused));
    }
}
