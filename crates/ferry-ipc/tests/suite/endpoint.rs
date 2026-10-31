use ferry_ipc::{Endpoint, VERSION};

use crate::support::TempDir;

fn sample(pid: u32) -> Endpoint {
    Endpoint {
        port: 51234,
        token: "ab".repeat(32),
        pid,
        version: VERSION,
    }
}

#[test]
fn saves_and_loads() {
    let dir = TempDir::new();
    let path = dir.path().join("sub/ipc.json");
    assert_eq!(Endpoint::load(&path).unwrap(), None);
    sample(7).save(&path).unwrap();
    assert_eq!(Endpoint::load(&path).unwrap(), Some(sample(7)));
}

#[test]
fn remove_only_deletes_its_own_file() {
    let dir = TempDir::new();
    let path = dir.path().join("ipc.json");
    sample(7).save(&path).unwrap();
    Endpoint::remove(&path, 8);
    assert!(path.exists(), "别的进程写的文件不能删");
    Endpoint::remove(&path, 7);
    assert!(!path.exists());
}
