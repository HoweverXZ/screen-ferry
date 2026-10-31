use ferry_core::KeyRemap;
use ferry_types::KeyCode;

#[test]
fn swaps_command_and_control_on_each_side() {
    let remap = KeyRemap::swap_command_control();
    assert_eq!(remap.apply(KeyCode::LEFT_GUI), KeyCode::LEFT_CTRL);
    assert_eq!(remap.apply(KeyCode::LEFT_CTRL), KeyCode::LEFT_GUI);
    assert_eq!(remap.apply(KeyCode::RIGHT_GUI), KeyCode::RIGHT_CTRL);
    assert_eq!(remap.apply(KeyCode::RIGHT_CTRL), KeyCode::RIGHT_GUI);
}

#[test]
fn leaves_other_keys_alone() {
    let letter_c = KeyCode::keyboard(0x06);
    for remap in [KeyRemap::none(), KeyRemap::swap_command_control()] {
        assert_eq!(remap.apply(letter_c), letter_c);
        assert_eq!(remap.apply(KeyCode::LEFT_ALT), KeyCode::LEFT_ALT);
    }
    assert_eq!(KeyRemap::none().apply(KeyCode::LEFT_GUI), KeyCode::LEFT_GUI);
}
