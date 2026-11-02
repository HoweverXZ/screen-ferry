use ferry_types::{Config, DeviceSettings, Lang, Language, ModifierPreset, Role, Side};

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
    assert_eq!(c.language, Language::System);
}

#[test]
fn the_language_follows_the_system_unless_chosen() {
    for (tag, lang) in [
        ("zh-Hans-CN", Lang::Zh),
        ("zh_TW", Lang::Zh),
        ("ZH", Lang::Zh),
        ("en-US", Lang::En),
        ("ja-JP", Lang::En),
        ("", Lang::En),
    ] {
        assert_eq!(Lang::from_tag(tag), lang, "{tag}");
    }
    assert_eq!(Language::System.resolve(Lang::En), Lang::En);
    assert_eq!(Language::Zh.resolve(Lang::En), Lang::Zh);
    assert_eq!(Language::En.resolve(Lang::Zh), Lang::En);
    assert_eq!(Lang::En.pick("中文", "English"), "English");
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
