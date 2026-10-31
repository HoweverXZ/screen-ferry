use std::fs;

use ferry_store::{Error, Store};
use ferry_trust::{Fingerprint, Peer, PeerList};

use crate::support::TempDir;

fn peer(name: &str, seed: u8) -> Peer {
    Peer {
        name: name.into(),
        fingerprint: Fingerprint::from_bytes([seed; 32]),
    }
}

#[test]
fn identity_is_created_once_and_then_reused() {
    let dir = TempDir::new();
    let store = Store::open(dir.path()).unwrap();
    let first = store.identity("Mac").unwrap();
    assert!(dir.path().join("identity/device.crt").is_file());
    assert!(dir.path().join("identity/device.key").is_file());

    let again = Store::open(dir.path()).unwrap().identity("改了名").unwrap();
    assert_eq!(again.fingerprint(), first.fingerprint());
}

#[test]
fn a_missing_key_is_an_error_not_a_silent_new_identity() {
    let dir = TempDir::new();
    let store = Store::open(dir.path()).unwrap();
    store.identity("Mac").unwrap();
    fs::remove_file(dir.path().join("identity/device.key")).unwrap();
    assert!(matches!(store.identity("Mac"), Err(Error::Identity { .. })));
}

#[test]
fn a_corrupted_identity_is_reported() {
    let dir = TempDir::new();
    let store = Store::open(dir.path()).unwrap();
    store.identity("Mac").unwrap();
    fs::write(dir.path().join("identity/device.key"), b"broken").unwrap();
    let err = store.identity("Mac").unwrap_err();
    assert!(err.to_string().contains("重新配对"), "{err}");
}

#[test]
fn peers_start_empty_and_round_trip() {
    let dir = TempDir::new();
    let store = Store::open(dir.path()).unwrap();
    assert!(store.peers().unwrap().is_empty());

    let list: PeerList = [peer("办公室 PC", 1), peer("家里", 2)]
        .into_iter()
        .collect();
    store.save_peers(&list).unwrap();
    assert_eq!(store.peers().unwrap(), list);

    let text = fs::read_to_string(dir.path().join("peers.toml")).unwrap();
    assert!(text.starts_with("schema_version = 1"), "{text}");
    assert!(text.contains("[[peer]]"), "{text}");
    assert!(!dir.path().join("peers.toml.tmp").exists());
}

#[test]
fn hand_edited_peers_file_is_read() {
    let dir = TempDir::new();
    let store = Store::open(dir.path()).unwrap();
    let fp = Fingerprint::from_bytes([0xAB; 32]);
    let text = format!(
        "schema_version = 1\n\n[[peer]]\nname = \"PC\"\nfingerprint = \"{}\"\n",
        fp.to_hex().to_uppercase()
    );
    fs::write(dir.path().join("peers.toml"), text).unwrap();
    assert!(store.peers().unwrap().contains(&fp));
}

#[test]
fn bad_peers_files_are_rejected() {
    let dir = TempDir::new();
    let store = Store::open(dir.path()).unwrap();
    let path = dir.path().join("peers.toml");

    fs::write(&path, "schema_version = 2\n").unwrap();
    assert!(matches!(store.peers(), Err(Error::TooNew { found: 2, .. })));

    fs::write(
        &path,
        "schema_version = 1\n[[peer]]\nname = \"x\"\nfingerprint = \"zz\"\n",
    )
    .unwrap();
    assert!(matches!(store.peers(), Err(Error::Format { .. })));

    fs::write(&path, "not toml at all [").unwrap();
    assert!(matches!(store.peers(), Err(Error::Format { .. })));
}
