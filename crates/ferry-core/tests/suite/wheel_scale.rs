use ferry_core::WheelScale;
use ferry_types::{DeviceSettings, ScrollSpeed};

fn wheel(edit: impl FnOnce(&mut DeviceSettings)) -> WheelScale {
    let mut settings = DeviceSettings::new("ab");
    edit(&mut settings);
    WheelScale::new(&settings)
}

#[test]
fn defaults_pass_the_wheel_through() {
    let mut w = wheel(|_| {});
    assert_eq!(w.apply(-30, 120), (-30, 120));
}

#[test]
fn each_direction_can_be_reversed_on_its_own() {
    let mut w = wheel(|s| s.reverse_vertical = true);
    assert_eq!(w.apply(-30, 120), (-30, -120));
    let mut w = wheel(|s| s.reverse_horizontal = true);
    assert_eq!(w.apply(-30, 120), (30, 120));
}

#[test]
fn speed_multiplies_and_carries_what_is_left() {
    let mut fast = wheel(|s| s.scroll_speed = ScrollSpeed::Fast);
    assert_eq!(fast.apply(0, 120), (0, 240));

    let mut slow = wheel(|s| s.scroll_speed = ScrollSpeed::Slow);
    assert_eq!(slow.apply(0, 3), (0, 1), "1.5 先给 1，余下 0.5");
    assert_eq!(slow.apply(0, 1), (0, 1), "0.5 + 0.5 凑够 1");
    assert_eq!(slow.apply(0, 1), (0, 0));
}

#[test]
fn turning_back_drops_what_was_left_from_the_other_way() {
    let mut slow = wheel(|s| s.scroll_speed = ScrollSpeed::Slow);
    assert_eq!(slow.apply(0, 1), (0, 0));
    assert_eq!(slow.apply(0, -1), (0, 0), "向上剩的半格不抵消向下");
    assert_eq!(slow.apply(0, -1), (0, -1));
}

#[test]
fn reversing_also_applies_to_the_carried_part() {
    let mut w = wheel(|s| {
        s.reverse_vertical = true;
        s.scroll_speed = ScrollSpeed::Slow;
    });
    assert_eq!(w.apply(0, 1), (0, 0));
    assert_eq!(w.apply(0, 1), (0, -1));
}
