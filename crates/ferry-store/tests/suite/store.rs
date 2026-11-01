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

#[test]
fn config_defaults_until_saved_then_round_trips() {
    use ferry_types::{Config, Side};

    let dir = TempDir::new();
    let store = Store::open(dir.path()).unwrap();
    assert_eq!(store.config().unwrap(), Config::default());
    assert!(!dir.path().join("config.toml").exists(), "只读不写");

    let config = Config {
        name: "书房 Mac".into(),
        side: Side::Left,
        clipboard: false,
        ..Config::default()
    };
    store.save_config(&config).unwrap();
    let text = fs::read_to_string(dir.path().join("config.toml")).unwrap();
    assert!(
        text.starts_with("schema_version = 3\n"),
        "第一行必须是 schema_version：{text}"
    );
    assert_eq!(store.config().unwrap(), config);
}

#[test]
fn config_tolerates_missing_and_unknown_fields() {
    let dir = TempDir::new();
    let store = Store::open(dir.path()).unwrap();
    fs::write(
        dir.path().join("config.toml"),
        "schema_version = 1\nside = \"top\"\nfuture_field = 3\n",
    )
    .unwrap();
    let config = store.config().unwrap();
    assert_eq!(config.side, ferry_types::Side::Top);
    assert!(config.clipboard, "缺少的字段取缺省值");
}

#[test]
fn config_from_a_newer_version_is_refused() {
    let dir = TempDir::new();
    let store = Store::open(dir.path()).unwrap();
    fs::write(dir.path().join("config.toml"), "schema_version = 4\n").unwrap();
    assert!(matches!(
        store.config(),
        Err(Error::TooNew { found: 4, .. })
    ));
}

#[test]
fn a_version_1_config_reads_as_version_2_with_an_empty_layout() {
    use ferry_types::{Config, Placement, Side};

    let dir = TempDir::new();
    let store = Store::open(dir.path()).unwrap();
    fs::write(
        dir.path().join("config.toml"),
        "schema_version = 1\nside = \"left\"\nclipboard = false\n",
    )
    .unwrap();
    let config = store.config().unwrap();
    assert_eq!(config.schema_version, Config::SCHEMA_VERSION);
    assert_eq!(
        config.side,
        Side::Left,
        "原来的一侧成为没有单独安排的设备的一侧"
    );
    assert!(config.layout.is_empty());
    assert!(!config.clipboard);

    // 安排过位置后写回：版本号更新，布局排在最后。
    let config = Config {
        layout: vec![
            Placement {
                fingerprint: "ab".repeat(32),
                side: Side::Right,
            },
            Placement {
                fingerprint: "cd".repeat(32),
                side: Side::Right,
            },
        ],
        ..config
    };
    store.save_config(&config).unwrap();
    let text = fs::read_to_string(dir.path().join("config.toml")).unwrap();
    assert!(text.starts_with("schema_version = 3\n"), "{text}");
    assert!(text.contains("[[layout]]"), "{text}");
    assert_eq!(store.config().unwrap(), config);
}

#[test]
fn a_custom_data_dir_keeps_its_runtime_files_inside() {
    let dir = TempDir::new();
    let store = Store::open(dir.path()).unwrap();
    assert_eq!(store.ipc_endpoint_path(), dir.path().join("ipc.json"));
}

#[test]
fn a_custom_data_dir_keeps_received_files_inside_it() {
    let dir = TempDir::new();
    let store = Store::open(dir.path()).unwrap();
    assert_eq!(store.downloads_dir(), dir.path().join("received"));
    assert_eq!(store.partial_dir(), dir.path().join("partial"));
}

#[test]
fn a_version_2_config_reads_as_version_3_with_the_old_clipboard_limit() {
    use ferry_types::Config;

    let dir = TempDir::new();
    let store = Store::open(dir.path()).unwrap();
    fs::write(
        dir.path().join("config.toml"),
        "schema_version = 2\nclipboard = true\n",
    )
    .unwrap();
    let config = store.config().unwrap();
    assert_eq!(config.schema_version, 3);
    assert_eq!(config.clipboard_limit_mb, 32, "与之前固定的上限相同");
    assert_eq!(config.clipboard_limit_bytes(), 32 * 1024 * 1024);

    let wild = Config {
        clipboard_limit_mb: 1000,
        ..Config::default()
    };
    assert_eq!(wild.clipboard_limit_bytes(), 64 * 1024 * 1024, "收进范围内");
}

#[test]
fn blobs_are_stored_by_content_and_can_be_removed() {
    let dir = TempDir::new();
    let store = Store::open(dir.path()).unwrap();
    let a = store.put_blob(b"picture").unwrap();
    assert_eq!(store.put_blob(b"picture").unwrap(), a, "同样的内容只存一份");
    assert_eq!(fs::read(&a).unwrap(), b"picture");
    assert!(a.starts_with(dir.path().join("blobs")));
    let name = a.file_name().unwrap().to_str().unwrap();
    let shard = a.parent().unwrap().file_name().unwrap().to_str().unwrap();
    assert_eq!(shard.len() + name.len(), 64, "名字是 SHA-256 的十六进制");

    let b = store.put_blob(b"another").unwrap();
    store.remove_blob(&a).unwrap();
    assert!(!a.exists());
    store.remove_blob(&a).unwrap();

    let outside = dir.path().join("config.toml");
    fs::write(&outside, "x").unwrap();
    store.remove_blob(&outside).unwrap();
    assert!(outside.exists(), "blobs/ 以外的文件不碰");

    store.clear_blobs().unwrap();
    assert!(!b.exists());
    store.clear_blobs().unwrap();
}
