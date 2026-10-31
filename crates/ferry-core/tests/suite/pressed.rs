use ferry_core::Pressed;
use ferry_types::{InputEvent, KeyAction, KeyCode, MouseButton, Point};

fn key(code: KeyCode, action: KeyAction) -> InputEvent {
    InputEvent::Key { code, action }
}

fn button(button: MouseButton, pressed: bool) -> InputEvent {
    InputEvent::Button {
        button,
        pressed,
        position: Point::default(),
    }
}

#[test]
fn releases_what_is_still_held() {
    let letter_a = KeyCode::keyboard(0x04);
    let letter_b = KeyCode::keyboard(0x05);
    let mut pressed = Pressed::new();
    for event in [
        key(KeyCode::LEFT_CTRL, KeyAction::Down),
        key(letter_a, KeyAction::Down),
        key(letter_b, KeyAction::Down),
        key(letter_b, KeyAction::Up),
        button(MouseButton::LEFT, true),
    ] {
        pressed.track(&event);
    }

    let at = Point::new(5, 6);
    assert_eq!(
        pressed.release_all(at),
        [
            key(letter_a, KeyAction::Up),
            key(KeyCode::LEFT_CTRL, KeyAction::Up),
            InputEvent::Button {
                button: MouseButton::LEFT,
                pressed: false,
                position: at
            },
        ],
        "普通键先松，修饰键其次，鼠标按键最后",
    );
    assert!(pressed.is_empty());
    assert!(pressed.release_all(at).is_empty());
}

#[test]
fn repeat_counts_as_held() {
    let mut pressed = Pressed::new();
    pressed.track(&key(KeyCode::keyboard(0x04), KeyAction::Repeat));
    assert!(!pressed.is_empty());
}

#[test]
fn tracks_buttons_separately_from_keys() {
    let mut pressed = Pressed::new();
    pressed.track(&key(KeyCode::LEFT_SHIFT, KeyAction::Down));
    assert!(!pressed.any_button());
    pressed.track(&button(MouseButton::RIGHT, true));
    assert!(pressed.any_button());
    pressed.track(&button(MouseButton::RIGHT, false));
    assert!(!pressed.any_button());
}

#[test]
fn clear_keys_keeps_buttons() {
    let mut pressed = Pressed::new();
    pressed.track(&key(KeyCode::LEFT_CTRL, KeyAction::Down));
    pressed.track(&button(MouseButton::LEFT, true));
    assert!(pressed.is_held(KeyCode::LEFT_CTRL));

    pressed.clear_keys();
    assert!(!pressed.is_held(KeyCode::LEFT_CTRL));
    assert!(pressed.any_button());
}
