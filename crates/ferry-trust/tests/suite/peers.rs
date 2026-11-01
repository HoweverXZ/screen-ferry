use ferry_trust::{Fingerprint, Peer, PeerList};

fn peer(name: &str, seed: u8) -> Peer {
    Peer::new(name, Fingerprint::from_bytes([seed; 32]))
}

#[test]
fn adding_the_same_fingerprint_only_renames() {
    let mut list = PeerList::new();
    list.add(peer("旧名字", 1));
    list.add(peer("新名字", 1));
    assert_eq!(list.len(), 1);
    assert_eq!(list.get(&peer("", 1).fingerprint).unwrap().name, "新名字");
}

#[test]
fn removal_reports_whether_anything_changed() {
    let mut list: PeerList = [peer("a", 1), peer("b", 2)].into_iter().collect();
    assert!(list.remove(&peer("", 1).fingerprint));
    assert!(!list.remove(&peer("", 1).fingerprint));
    assert!(!list.contains(&peer("", 1).fingerprint));
    assert!(list.contains(&peer("", 2).fingerprint));
}

#[test]
fn order_is_kept() {
    let list: PeerList = [peer("a", 3), peer("b", 1), peer("c", 2)]
        .into_iter()
        .collect();
    let names: Vec<_> = list.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["a", "b", "c"]);
}

#[test]
fn the_last_address_is_remembered_and_keeps_a_known_port() {
    use std::net::SocketAddr;

    let mut list = PeerList::new();
    let pc = peer("PC", 1);
    list.add(pc.clone());
    let at = |s: &str| s.parse::<SocketAddr>().unwrap();

    assert!(list.set_addr(&pc.fingerprint, at("192.168.2.18:24810")));
    assert!(
        !list.set_addr(&pc.fingerprint, at("192.168.2.18:24810")),
        "没变化"
    );
    assert!(
        !list.set_addr(&pc.fingerprint, at("192.168.2.18:0")),
        "同一个 IP 只知道 IP 时沿用原来的端口"
    );
    assert_eq!(
        list.get(&pc.fingerprint).unwrap().addr,
        Some(at("192.168.2.18:24810"))
    );
    assert!(
        list.set_addr(&pc.fingerprint, at("192.168.2.30:0")),
        "换了 IP"
    );
    assert_eq!(
        list.get(&pc.fingerprint).unwrap().addr,
        Some(at("192.168.2.30:0"))
    );

    let stranger = peer("陌生", 9);
    assert!(
        !list.set_addr(&stranger.fingerprint, at("10.0.0.1:24810")),
        "不在列表里的不记"
    );
}

#[test]
fn adding_again_keeps_the_address_unless_a_new_one_is_given() {
    use std::net::SocketAddr;

    let mut list = PeerList::new();
    let mut pc = peer("PC", 1);
    pc.addr = Some("192.168.2.18:24810".parse::<SocketAddr>().unwrap());
    list.add(pc.clone());
    list.add(peer("改了名的 PC", 1));
    let saved = list.get(&pc.fingerprint).unwrap();
    assert_eq!(saved.name, "改了名的 PC");
    assert_eq!(saved.addr, pc.addr);
}
