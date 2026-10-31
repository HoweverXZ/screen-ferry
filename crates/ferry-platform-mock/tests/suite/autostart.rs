use ferry_platform::Autostart;
use ferry_platform_mock::MockAutostart;

#[test]
fn remembers_the_setting() {
    let autostart = MockAutostart::new();
    assert!(!autostart.is_enabled().unwrap());
    autostart.set_enabled(true).unwrap();
    assert!(
        autostart.clone().is_enabled().unwrap(),
        "克隆共用同一个开关"
    );
}
