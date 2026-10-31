use std::time::Duration;

use ferry_engine::Error;
use ferry_net::Tls;
use ferry_proto::{Capabilities, Channel, Hello, RejectReason};
use ferry_trust::{Identity, PeerList};
use tokio::time::timeout;

use crate::support::{AutoConfirm, Rig, Setup};

#[tokio::test]
async fn first_connection_pairs_after_both_sides_confirm() {
    let rig = Rig::start().await;
    rig.enter().await;

    let (mac_seen, pc_seen) = (rig.mac_confirm.requests(), rig.pc_confirm.requests());
    assert_eq!((mac_seen.len(), pc_seen.len()), (1, 1), "两边各确认一次");
    assert_eq!(mac_seen[0].code, pc_seen[0].code, "两边显示同一个配对码");
    assert_eq!(mac_seen[0].peer_name, "pc");
    assert_eq!(pc_seen[0].peer_name, "mac");

    let mac_id = rig.mac.store.identity("mac").unwrap();
    let pc_id = rig.pc.store.identity("pc").unwrap();
    assert_eq!(mac_seen[0].peer_fingerprint, pc_id.fingerprint());
    assert_eq!(pc_seen[0].peer_fingerprint, mac_id.fingerprint());
    assert!(
        rig.mac
            .store
            .peers()
            .unwrap()
            .contains(&pc_id.fingerprint())
    );
    assert!(
        rig.pc
            .store
            .peers()
            .unwrap()
            .contains(&mac_id.fingerprint())
    );
}

#[tokio::test]
async fn paired_devices_reconnect_without_asking_again() {
    let rig = Rig::start().await;
    rig.enter().await;
    let (mac, pc) = rig.stop();

    let rig = Rig::with(Setup {
        mac,
        pc,
        ..Setup::default()
    })
    .await;
    rig.enter().await;
    assert!(rig.mac_confirm.requests().is_empty());
    assert!(rig.pc_confirm.requests().is_empty());
}

#[tokio::test]
async fn the_server_saying_no_stops_the_client() {
    let rig = Rig::with(Setup {
        mac_confirm: AutoConfirm::answering(false),
        ..Setup::default()
    })
    .await;

    let error = timeout(Duration::from_secs(5), rig.client)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(error, Error::PairingRejected), "{error}");
    assert!(rig.mac.store.peers().unwrap().is_empty());
    assert!(rig.pc.store.peers().unwrap().is_empty());
    assert!(!rig.capture.is_grabbed());
}

#[tokio::test]
async fn the_client_saying_no_also_stops_it() {
    let rig = Rig::with(Setup {
        pc_confirm: AutoConfirm::answering(false),
        ..Setup::default()
    })
    .await;

    let error = timeout(Duration::from_secs(5), rig.client)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(error, Error::PairingRejected), "{error}");
    assert!(rig.mac.store.peers().unwrap().is_empty());
    assert!(rig.pc.store.peers().unwrap().is_empty());
}

#[tokio::test]
async fn a_client_the_server_forgot_pairs_again() {
    let rig = Rig::start().await;
    rig.enter().await;
    let (mac, pc) = rig.stop();
    // Mac 上删掉了这台 PC，PC 还记得 Mac。
    mac.store.save_peers(&PeerList::new()).unwrap();

    let rig = Rig::with(Setup {
        mac,
        pc,
        ..Setup::default()
    })
    .await;
    rig.enter().await;
    assert_eq!(rig.mac_confirm.requests().len(), 1);
    assert_eq!(rig.pc_confirm.requests().len(), 1);
}

#[tokio::test]
async fn a_stranger_cannot_start_a_session() {
    let rig = Rig::with(Setup {
        start_client: false,
        ..Setup::default()
    })
    .await;

    let stranger = Tls::new(&Identity::generate("陌生人").unwrap()).unwrap();
    let (mut framed, _) = stranger.connect(rig.addr, Channel::Input).await.unwrap();
    let hello = Hello::new(Channel::Input, Capabilities::all(), 0, "陌生人");
    match ferry_net::initiate(&mut framed, hello).await {
        Err(ferry_net::Error::Rejected(r)) => assert_eq!(r.reason, RejectReason::NOT_PAIRED),
        other => panic!("应被拒绝，实际 {other:?}"),
    }
    assert!(rig.mac_confirm.requests().is_empty(), "不配对就不打扰用户");
}

#[tokio::test]
async fn a_stranger_cannot_join_someone_elses_session() {
    let rig = Rig::start().await;
    rig.enter().await;

    let stranger = Tls::new(&Identity::generate("陌生人").unwrap()).unwrap();
    // 会话号是递增的小整数，很好猜；挡住它的是证书指纹。
    for session in 1..=4 {
        let (mut framed, _) = stranger.connect(rig.addr, Channel::Data).await.unwrap();
        let hello = Hello::new(Channel::Data, Capabilities::all(), session, "陌生人");
        match ferry_net::initiate(&mut framed, hello).await {
            Err(ferry_net::Error::Rejected(r)) => {
                assert_eq!(r.reason, RejectReason::UNKNOWN_SESSION);
            }
            other => panic!("会话 {session} 应被拒绝，实际 {other:?}"),
        }
    }
    // 原来的会话不受影响。
    rig.leave().await;
    rig.enter().await;
}

/// 不填地址：服务端广播，客户端在局域网里找到它、配对、开始会话。
/// 需要本机网络支持组播，默认跳过：`cargo test -p ferry-engine -- --ignored`
#[tokio::test]
#[ignore = "需要本机网络支持组播"]
async fn a_client_without_an_address_finds_and_pairs_with_the_server() {
    let rig = Rig::with(Setup {
        discover: true,
        ..Setup::default()
    })
    .await;
    // 发现需要一点时间，多推几次。
    for _ in 0..5 {
        if rig.mac_confirm.requests().len() == 1 {
            break;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    rig.enter().await;
    assert_eq!(rig.pc_confirm.requests().len(), 1);
}
