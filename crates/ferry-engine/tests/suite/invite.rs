use std::net::SocketAddr;

use ferry_engine::{DialError, Event, invite};

use crate::support::{Rig, Setup};

async fn server_only() -> Rig {
    Rig::with(Setup {
        start_client: false,
        ..Setup::default()
    })
    .await
}

#[tokio::test]
async fn an_invited_client_connects_to_the_server() {
    let rig = server_only().await;
    let mut pc = rig.idle_client("书房", true);
    let port = pc
        .wait_for("开始等邀请", |e| match e {
            Event::InviteListening { port } => Some(*port),
            _ => None,
        })
        .await;

    let to = SocketAddr::from(([127, 0, 0, 1], port));
    let accepted = invite(&rig.mac.store, "mac", to, rig.addr.port())
        .await
        .expect("对方接受邀请");
    assert_eq!(accepted.to_hex(), pc.fingerprint("书房"));

    // 没配对过：两边核对配对码（测试里自动同意），然后连上。
    pc.wait_for("应邀连上服务端", |e| {
        matches!(e, Event::Connected { .. }).then_some(())
    })
    .await;
}

#[tokio::test]
async fn a_server_does_not_take_invitations() {
    let rig = server_only().await;
    let error = invite(&rig.mac.store, "mac", rig.addr, 24810)
        .await
        .expect_err("服务端不等邀请");
    assert_eq!(DialError::of(&error), DialError::Refused);
}

#[tokio::test]
async fn inviting_where_nothing_waits_says_so() {
    let rig = server_only().await;
    let closed = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap()
    };
    let error = invite(&rig.mac.store, "mac", closed, rig.addr.port())
        .await
        .expect_err("没人在等");
    assert_eq!(DialError::of(&error), DialError::Refused);
}
