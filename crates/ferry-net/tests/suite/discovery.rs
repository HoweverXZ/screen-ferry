use std::time::{Duration, Instant};

use ferry_net::discovery::{Advertiser, Browser, Discovered, Lookup, pick};
use ferry_trust::{Fingerprint, Peer, PeerList};

fn server(name: &str, seed: u8) -> Discovered {
    Discovered {
        name: name.into(),
        fingerprint: Fingerprint::from_bytes([seed; 32]),
        addrs: vec![([192, 168, 1, seed], 24810).into()],
    }
}

fn trusting(seeds: &[u8]) -> PeerList {
    seeds
        .iter()
        .map(|&seed| Peer {
            name: format!("设备 {seed}"),
            fingerprint: Fingerprint::from_bytes([seed; 32]),
        })
        .collect()
}

#[test]
fn a_paired_server_wins_over_strangers() {
    let found = [
        server("陌生", 1),
        server("家里的 Mac", 2),
        server("又一台", 3),
    ];
    assert_eq!(
        pick(&found, &trusting(&[2])),
        Lookup::Trusted(server("家里的 Mac", 2))
    );
}

#[test]
fn the_only_server_is_chosen_when_nothing_is_paired_yet() {
    assert_eq!(
        pick(&[server("Mac", 1)], &PeerList::new()),
        Lookup::Unpaired(server("Mac", 1))
    );
}

#[test]
fn several_unpaired_servers_are_not_guessed() {
    let found = [server("a", 1), server("b", 2)];
    assert_eq!(
        pick(&found, &PeerList::new()),
        Lookup::Nothing {
            others: found.to_vec()
        }
    );
}

#[test]
fn strangers_are_ignored_once_something_is_paired() {
    // 已配对的那台不在线时，不能自动连上局域网里的另一台。
    let found = [server("别人的 Mac", 1)];
    assert_eq!(
        pick(&found, &trusting(&[9])),
        Lookup::Nothing {
            others: found.to_vec()
        }
    );
}

#[test]
fn nothing_found() {
    assert_eq!(
        pick(&[], &PeerList::new()),
        Lookup::Nothing { others: vec![] }
    );
}

/// 真实的组播收发。CI 虚拟机里的组播不一定可用，所以默认跳过：
/// `cargo test -p ferry-net -- --ignored`
#[test]
#[ignore = "需要本机网络支持组播"]
fn an_advertised_server_is_found_on_this_machine() {
    let fingerprint = Fingerprint::of(b"discovery test");
    let _advertiser = Advertiser::start("测试 Mac", fingerprint, 45_678).unwrap();
    let browser = Browser::start().unwrap();

    let deadline = Instant::now() + Duration::from_secs(10);
    let server = loop {
        if let Some(s) = browser
            .found()
            .into_iter()
            .find(|s| s.fingerprint == fingerprint)
        {
            break s;
        }
        assert!(Instant::now() < deadline, "10 秒内没有发现自己广播的服务");
        std::thread::sleep(Duration::from_millis(100));
    };
    assert_eq!(server.name, "测试 Mac");
    assert!(server.addrs.iter().all(|a| a.port() == 45_678));
    assert!(!server.addrs.is_empty());
}
