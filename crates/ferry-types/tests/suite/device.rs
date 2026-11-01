use ferry_types::{DeviceSettings, ModifierMap, ModifierPreset, ModifierTarget, ScrollSpeed};

#[test]
fn defaults_change_nothing() {
    let d = DeviceSettings::new("ab");
    assert_eq!(d.pointer_dpi, 0);
    assert!(!d.reverse_vertical && !d.reverse_horizontal);
    assert_eq!(d.scroll_speed, ScrollSpeed::Normal);
    assert_eq!(d.modifiers, ModifierPreset::Habit);
    assert!(!d.text_shortcuts);
    assert_eq!(d.edge_delay_ms, 0);
    assert!(d.clipboard && d.files);
    assert!(d.keys.is_empty());
}

#[test]
fn habit_swaps_command_and_ctrl_only_between_different_keyboards() {
    let d = DeviceSettings::new("ab");
    assert_eq!(d.modifier_map(true), ModifierMap::SWAP_GUI_CTRL);
    assert_eq!(d.modifier_map(false), ModifierMap::PLAIN);
}

#[test]
fn plain_and_custom_ignore_the_keyboards() {
    let mut d = DeviceSettings::new("ab");
    d.modifiers = ModifierPreset::Plain;
    assert_eq!(d.modifier_map(true), ModifierMap::PLAIN);

    d.modifiers = ModifierPreset::Custom;
    d.custom_modifiers.caps_lock = ModifierTarget::Ctrl;
    let map = d.modifier_map(false);
    assert_eq!(map.caps_lock, ModifierTarget::Ctrl);
    assert_eq!(map.gui, ModifierTarget::Gui);
}

#[test]
fn out_of_range_values_are_pulled_back() {
    let mut d = DeviceSettings::new("ab");
    d.pointer_dpi = 20;
    d.edge_delay_ms = 99_999;
    let d = d.clamped();
    assert_eq!(d.pointer_dpi, *DeviceSettings::POINTER_DPI.start());
    assert_eq!(d.edge_delay_ms, DeviceSettings::MAX_EDGE_DELAY_MS);

    // 0 表示“和本机一样”，不收。
    let d = DeviceSettings::new("ab").clamped();
    assert_eq!(d.pointer_dpi, 0);
}

#[test]
fn scroll_speeds_scale_by_these_percentages() {
    assert_eq!(ScrollSpeed::Slow.percent(), 50);
    assert_eq!(ScrollSpeed::Normal.percent(), 100);
    assert_eq!(ScrollSpeed::Fast.percent(), 200);
}
