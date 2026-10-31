use std::fs;

use ferry_platform::Autostart;
use ferry_platform_macos::MacAutostart;

#[test]
fn writes_and_removes_a_launch_agent() {
    let dir = std::env::temp_dir().join(format!("ferry-autostart-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let autostart = MacAutostart::in_dir(
        dir.clone(),
        "io.example.test",
        vec![
            "/Applications/A & B.app/Contents/MacOS/x".into(),
            "--autostart".into(),
        ],
    );
    assert!(!autostart.is_enabled().unwrap());
    autostart.set_enabled(false).unwrap(); // 没开过也可以关，不报错

    autostart.set_enabled(true).unwrap();
    assert!(autostart.is_enabled().unwrap());
    let plist = fs::read_to_string(dir.join("io.example.test.plist")).unwrap();
    assert!(plist.contains("<string>io.example.test</string>"));
    assert!(plist.contains("<string>/Applications/A &amp; B.app/Contents/MacOS/x</string>"));
    assert!(plist.contains("<string>--autostart</string>"));
    assert!(plist.contains("<key>RunAtLoad</key>\n\t<true/>"));

    autostart.set_enabled(false).unwrap();
    assert!(!autostart.is_enabled().unwrap());
    let _ = fs::remove_dir_all(&dir);
}
