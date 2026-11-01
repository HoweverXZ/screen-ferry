use ferry_types::{Config, DeviceSettings, ModifierPreset, Role, Side};

#[test]
fn defaults_match_the_documented_config() {
    let c = Config::default();
    assert_eq!(c.schema_version, Config::SCHEMA_VERSION);
    assert_eq!(c.side, Side::Right);
    assert!(c.announce && c.clipboard);
    assert_eq!(c.mouse_dpi, 1600);
    assert!(c.devices.is_empty());
    assert!(c.name.is_empty() && c.server.is_empty());
    assert_eq!(c.role, Role::native());
}

#[test]
fn a_device_without_its_own_settings_gets_the_defaults() {
    let c = Config::default();
    let d = c.device("ab");
    assert_eq!(d, DeviceSettings::new("ab"));
    assert!(d.is_default());
}

#[test]
fn setting_a_device_replaces_it_and_drops_it_once_back_to_defaults() {
    let mut c = Config::default();
    let mut d = DeviceSettings::new("ab");
    d.reverse_vertical = true;
    c.set_device(d.clone());
    assert_eq!(c.devices, vec![d.clone()]);

    d.modifiers = ModifierPreset::Plain;
    c.set_device(d.clone());
    assert_eq!(c.device("ab"), d);
    assert_eq!(c.devices.len(), 1);

    c.set_device(DeviceSettings::new("ab"));
    assert!(c.devices.is_empty());
}

#[test]
fn mouse_dpi_is_kept_within_range() {
    let mut c = Config {
        mouse_dpi: 5,
        ..Config::default()
    };
    assert_eq!(c.mouse_dpi(), *Config::MOUSE_DPI.start());
    c.mouse_dpi = 1_000_000;
    assert_eq!(c.mouse_dpi(), *Config::MOUSE_DPI.end());
}
