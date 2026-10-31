use ferry_types::MouseButton;

#[test]
fn zero_is_not_a_button() {
    assert_eq!(MouseButton::new(0), None);
}

#[test]
fn buttons_are_numbered_from_one() {
    let standard = [
        MouseButton::LEFT,
        MouseButton::RIGHT,
        MouseButton::MIDDLE,
        MouseButton::BACK,
        MouseButton::FORWARD,
    ];
    let numbers: Vec<u8> = standard.iter().map(|b| b.number()).collect();
    assert_eq!(numbers, [1, 2, 3, 4, 5]);
    assert_eq!(MouseButton::new(8).map(MouseButton::number), Some(8));
}
