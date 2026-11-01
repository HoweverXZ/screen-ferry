use ferry_core::KeyMapper;
use ferry_types::{DeviceSettings, KeyAction, KeyCode, KeyMapping, ModifierPreset, ModifierTarget};

const C: KeyCode = KeyCode::keyboard(0x06);
const LEFT: KeyCode = KeyCode::keyboard(0x50);
const RIGHT: KeyCode = KeyCode::keyboard(0x4F);
const UP: KeyCode = KeyCode::keyboard(0x52);
const HOME: KeyCode = KeyCode::keyboard(0x4A);
const END: KeyCode = KeyCode::keyboard(0x4D);
const BACKSPACE: KeyCode = KeyCode::keyboard(0x2A);
const F13: KeyCode = KeyCode::keyboard(0x68);
const PRINT_SCREEN: KeyCode = KeyCode::keyboard(0x46);

const CMD: KeyCode = KeyCode::LEFT_GUI;
const CTRL: KeyCode = KeyCode::LEFT_CTRL;
const ALT: KeyCode = KeyCode::LEFT_ALT;
const SHIFT: KeyCode = KeyCode::LEFT_SHIFT;

fn down(code: KeyCode) -> (KeyCode, KeyAction) {
    (code, KeyAction::Down)
}

fn up(code: KeyCode) -> (KeyCode, KeyAction) {
    (code, KeyAction::Up)
}

fn repeat(code: KeyCode) -> (KeyCode, KeyAction) {
    (code, KeyAction::Repeat)
}

/// 依次按下、松开这些键，收集发给对方的按键。
fn press(m: &mut KeyMapper, keys: &[(KeyCode, KeyAction)]) -> Vec<(KeyCode, KeyAction)> {
    keys.iter()
        .flat_map(|&(code, action)| m.key(code, action))
        .collect()
}

fn settings() -> DeviceSettings {
    DeviceSettings::new("ab")
}

/// 打开了文本编辑快捷键的设置。
fn shortcuts() -> DeviceSettings {
    DeviceSettings {
        text_shortcuts: true,
        ..settings()
    }
}

/// Mac 控制 Windows。
fn mac_to_pc(settings: &DeviceSettings) -> KeyMapper {
    KeyMapper::new(settings, true, false)
}

/// Windows 控制 Mac。
fn pc_to_mac(settings: &DeviceSettings) -> KeyMapper {
    KeyMapper::new(settings, false, true)
}

// ── 修饰键与其他映射 ─────────────────────────

#[test]
fn habit_swaps_command_and_ctrl_between_different_keyboards() {
    let mut m = mac_to_pc(&settings());
    assert_eq!(
        press(&mut m, &[down(CMD), down(C), up(C), up(CMD)]),
        [down(CTRL), down(C), up(C), up(CTRL)],
        "⌘C 到了 Windows 上是 Ctrl+C",
    );
    assert_eq!(
        press(
            &mut m,
            &[down(KeyCode::RIGHT_GUI), down(KeyCode::RIGHT_CTRL)]
        ),
        [down(KeyCode::RIGHT_CTRL), down(KeyCode::RIGHT_GUI)],
        "左右各自对应",
    );
}

#[test]
fn habit_keeps_keys_between_the_same_kind_of_keyboard() {
    let mut m = KeyMapper::new(&settings(), true, true);
    assert_eq!(
        press(&mut m, &[down(CMD), down(CTRL)]),
        [down(CMD), down(CTRL)]
    );
}

#[test]
fn plain_sends_every_key_as_it_is() {
    let plain = DeviceSettings {
        modifiers: ModifierPreset::Plain,
        ..settings()
    };
    let mut m = mac_to_pc(&plain);
    assert_eq!(
        press(&mut m, &[down(CMD), down(C)]),
        [down(CMD), down(C)],
        "⌘ 到了 Windows 上就是 Windows 键",
    );
}

#[test]
fn custom_modifiers_can_change_or_drop_each_key() {
    let mut custom = DeviceSettings {
        modifiers: ModifierPreset::Custom,
        ..settings()
    };
    custom.custom_modifiers.caps_lock = ModifierTarget::Ctrl;
    custom.custom_modifiers.alt = ModifierTarget::None;
    custom.custom_modifiers.gui = ModifierTarget::Alt;
    let mut m = mac_to_pc(&custom);
    assert_eq!(
        press(
            &mut m,
            &[
                down(KeyCode::CAPS_LOCK),
                down(ALT),
                down(KeyCode::RIGHT_GUI),
                up(ALT),
                up(KeyCode::RIGHT_GUI),
                up(KeyCode::CAPS_LOCK),
            ]
        ),
        [
            down(CTRL),
            down(KeyCode::RIGHT_ALT),
            up(KeyCode::RIGHT_ALT),
            up(CTRL),
        ],
        "Caps Lock 当 Ctrl；⌥ 不发送；右 ⌘ 变成右 Alt",
    );
}

#[test]
fn other_mappings_come_before_the_modifiers_and_can_block_a_key() {
    let mut s = settings();
    s.keys = vec![
        KeyMapping {
            from: F13.raw(),
            to: PRINT_SCREEN.raw(),
        },
        KeyMapping {
            from: CMD.raw(),
            to: 0,
        },
        // 同一个键只看第一条。
        KeyMapping {
            from: F13.raw(),
            to: C.raw(),
        },
    ];
    let mut m = mac_to_pc(&s);
    assert_eq!(
        press(
            &mut m,
            &[
                down(F13),
                repeat(F13),
                up(F13),
                down(CMD),
                repeat(CMD),
                up(CMD)
            ]
        ),
        [down(PRINT_SCREEN), repeat(PRINT_SCREEN), up(PRINT_SCREEN)],
        "F13 换成 Print Screen；⌘ 不发送，按下、按住、松开都不发",
    );
}

#[test]
fn keys_held_before_the_cursor_arrived_are_released_with_the_current_mapping() {
    let mut m = mac_to_pc(&settings());
    assert_eq!(press(&mut m, &[up(CMD)]), [up(CTRL)]);
}

// ── Mac 控制 Windows 的快捷键 ─────────────────

#[test]
fn command_left_goes_to_the_start_of_the_line() {
    let mut m = mac_to_pc(&shortcuts());
    assert_eq!(
        press(&mut m, &[down(CMD), down(LEFT), up(LEFT), up(CMD)]),
        [down(CTRL), up(CTRL), down(HOME), up(HOME)],
        "先在对方松开 Ctrl，再按 Home；松开 ⌘ 时 Ctrl 早已松开，不再发",
    );
    assert_eq!(
        press(&mut m, &[down(CMD), down(RIGHT), up(RIGHT), up(CMD)]),
        [down(CTRL), up(CTRL), down(END), up(END)],
    );
}

#[test]
fn the_command_key_comes_back_for_the_next_ordinary_key() {
    let mut m = mac_to_pc(&shortcuts());
    assert_eq!(
        press(
            &mut m,
            &[down(CMD), down(LEFT), up(LEFT), down(C), up(C), up(CMD)]
        ),
        [
            down(CTRL),
            up(CTRL),
            down(HOME),
            up(HOME),
            down(CTRL),
            down(C),
            up(C),
            up(CTRL),
        ],
        "⌘← 之后接着按 ⌘C，仍是复制",
    );
}

#[test]
fn shift_stays_held_to_select() {
    let mut m = mac_to_pc(&shortcuts());
    assert_eq!(
        press(
            &mut m,
            &[
                down(SHIFT),
                down(CMD),
                down(LEFT),
                up(LEFT),
                up(CMD),
                up(SHIFT)
            ]
        ),
        [
            down(SHIFT),
            down(CTRL),
            up(CTRL),
            down(HOME),
            up(HOME),
            up(SHIFT)
        ],
        "⇧⌘← 选中到行首",
    );
}

#[test]
fn holding_the_key_repeats_the_target_key() {
    let mut m = mac_to_pc(&shortcuts());
    assert_eq!(
        press(
            &mut m,
            &[down(CMD), down(LEFT), repeat(LEFT), down(LEFT), up(LEFT)]
        ),
        [
            down(CTRL),
            up(CTRL),
            down(HOME),
            repeat(HOME),
            repeat(HOME),
            up(HOME)
        ],
        "按住时有的系统发 Repeat，有的再发 Down",
    );
}

#[test]
fn letting_go_of_the_modifier_first_still_releases_cleanly() {
    let mut m = mac_to_pc(&shortcuts());
    assert_eq!(
        press(&mut m, &[down(CMD), down(LEFT), up(CMD), up(LEFT)]),
        [down(CTRL), up(CTRL), down(HOME), up(HOME)],
    );
    // 之后的按键与快捷键无关。
    assert_eq!(press(&mut m, &[down(C), up(C)]), [down(C), up(C)]);
}

#[test]
fn option_arrows_move_by_word() {
    let mut m = mac_to_pc(&shortcuts());
    assert_eq!(
        press(&mut m, &[down(ALT), down(LEFT), up(LEFT), up(ALT)]),
        [
            down(ALT),
            down(CTRL),
            up(ALT),
            down(LEFT),
            up(LEFT),
            up(CTRL)
        ],
        "先按下 Ctrl 再松开 Alt，Windows 不会把它当成单独按了一下 Alt 去激活菜单栏",
    );
    assert_eq!(
        press(&mut m, &[down(ALT), down(BACKSPACE), up(BACKSPACE)]),
        [
            down(ALT),
            down(CTRL),
            up(ALT),
            down(BACKSPACE),
            up(BACKSPACE),
            up(CTRL)
        ],
        "⌥⌫ 删一个词",
    );
}

#[test]
fn command_up_goes_to_the_start_of_the_document() {
    let mut m = mac_to_pc(&shortcuts());
    assert_eq!(
        press(&mut m, &[down(CMD), down(UP), up(UP)]),
        [down(CTRL), down(HOME), up(HOME)],
        "Ctrl 正好就是要按的，留着用",
    );
    assert_eq!(press(&mut m, &[up(CMD)]), [up(CTRL)]);
}

#[test]
fn command_backspace_deletes_to_the_start_of_the_line() {
    let mut m = mac_to_pc(&shortcuts());
    assert_eq!(
        press(
            &mut m,
            &[down(CMD), down(BACKSPACE), up(BACKSPACE), up(CMD)]
        ),
        [
            down(CTRL),
            up(CTRL),
            down(SHIFT),
            down(HOME),
            up(HOME),
            up(SHIFT),
            down(BACKSPACE),
            up(BACKSPACE),
        ],
    );
}

#[test]
fn letting_go_of_a_lone_windows_key_is_masked() {
    let plain = DeviceSettings {
        modifiers: ModifierPreset::Plain,
        ..shortcuts()
    };
    let mut m = mac_to_pc(&plain);
    assert_eq!(
        press(&mut m, &[down(CMD), down(LEFT), up(LEFT), up(CMD)]),
        [
            down(CMD),
            down(CTRL),
            up(CTRL),
            up(CMD),
            down(HOME),
            up(HOME)
        ],
        "⌘ 原样是 Windows 键：先轻点一下 Ctrl 再松开它，开始菜单不会弹出来",
    );
}

#[test]
fn another_shortcut_while_one_is_held_switches_over() {
    let mut m = mac_to_pc(&shortcuts());
    assert_eq!(
        press(
            &mut m,
            &[down(CMD), down(LEFT), down(RIGHT), up(LEFT), up(RIGHT)]
        ),
        [
            down(CTRL),
            up(CTRL),
            down(HOME),
            up(HOME),
            down(END),
            up(END)
        ],
    );
}

#[test]
fn an_ordinary_key_while_a_shortcut_is_held_ends_it() {
    let mut m = mac_to_pc(&shortcuts());
    assert_eq!(
        press(&mut m, &[down(CMD), down(LEFT), down(C), up(LEFT), up(C)]),
        [
            down(CTRL),
            up(CTRL),
            down(HOME),
            up(HOME),
            down(CTRL),
            down(C),
            up(C)
        ],
    );
}

#[test]
fn several_kinds_of_modifiers_are_not_translated() {
    let mut m = mac_to_pc(&shortcuts());
    assert_eq!(
        press(&mut m, &[down(CMD), down(ALT), down(LEFT)]),
        [down(CTRL), down(ALT), down(LEFT)],
        "⌘⌥← 不是文本编辑快捷键",
    );
}

#[test]
fn no_translation_when_off_or_between_the_same_kind_of_keyboard() {
    let mut off = mac_to_pc(&settings());
    assert_eq!(
        press(&mut off, &[down(CMD), down(LEFT)]),
        [down(CTRL), down(LEFT)]
    );
    let mut same = KeyMapper::new(&shortcuts(), true, true);
    assert_eq!(
        press(&mut same, &[down(CMD), down(LEFT)]),
        [down(CMD), down(LEFT)]
    );
}

#[test]
fn reset_forgets_the_keys_held_on_the_other_side() {
    let mut m = mac_to_pc(&shortcuts());
    press(&mut m, &[down(CMD), down(LEFT)]);
    m.reset();
    assert_eq!(
        press(&mut m, &[down(LEFT), up(LEFT)]),
        [down(LEFT), up(LEFT)],
        "光标离开后 ⌘ 不再算按着",
    );
}

// ── Windows 控制 Mac 的快捷键 ─────────────────

#[test]
fn home_and_end_become_command_arrows_on_a_mac() {
    let mut m = pc_to_mac(&shortcuts());
    assert_eq!(
        press(&mut m, &[down(HOME), up(HOME), down(END), up(END)]),
        [
            down(CMD),
            down(LEFT),
            up(LEFT),
            up(CMD),
            down(CMD),
            down(RIGHT),
            up(RIGHT),
            up(CMD),
        ],
    );
}

#[test]
fn ctrl_arrows_become_option_arrows_on_a_mac() {
    let mut m = pc_to_mac(&shortcuts());
    assert_eq!(
        press(&mut m, &[down(CTRL), down(LEFT), up(LEFT), up(CTRL)]),
        [down(CMD), down(ALT), up(CMD), down(LEFT), up(LEFT), up(ALT)],
        "Ctrl 本来变成 ⌘：先按下 ⌥，再松开 ⌘",
    );
}

#[test]
fn ctrl_home_becomes_command_up_on_a_mac() {
    let mut m = pc_to_mac(&shortcuts());
    assert_eq!(
        press(&mut m, &[down(CTRL), down(HOME), up(HOME), up(CTRL)]),
        [down(CMD), down(UP), up(UP), up(CMD)],
    );
}
