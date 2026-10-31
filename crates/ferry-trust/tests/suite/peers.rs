use ferry_trust::{Fingerprint, Peer, PeerList};

fn peer(name: &str, seed: u8) -> Peer {
    Peer {
        name: name.into(),
        fingerprint: Fingerprint::from_bytes([seed; 32]),
    }
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
