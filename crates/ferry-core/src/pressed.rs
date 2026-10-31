//! 按键状态：记录按下但尚未松开的按键与鼠标按键，离开或断线时全部释放，杜绝卡键。

use std::collections::BTreeSet;

use ferry_types::{InputEvent, KeyAction, KeyCode, MouseButton, Point};

#[derive(Clone, Debug, Default)]
pub struct Pressed {
    keys: BTreeSet<KeyCode>,
    buttons: BTreeSet<MouseButton>,
}

impl Pressed {
    pub fn new() -> Self {
        Self::default()
    }

    /// 记录一个事件对按键状态的影响。其他事件忽略。
    pub fn track(&mut self, event: &InputEvent) {
        match *event {
            InputEvent::Key {
                code,
                action: KeyAction::Down | KeyAction::Repeat,
            } => {
                self.keys.insert(code);
            }
            InputEvent::Key {
                code,
                action: KeyAction::Up,
            } => {
                self.keys.remove(&code);
            }
            InputEvent::Button {
                button,
                pressed: true,
                ..
            } => {
                self.buttons.insert(button);
            }
            InputEvent::Button {
                button,
                pressed: false,
                ..
            } => {
                self.buttons.remove(&button);
            }
            _ => {}
        }
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty() && self.buttons.is_empty()
    }

    pub fn any_button(&self) -> bool {
        !self.buttons.is_empty()
    }

    /// 生成释放全部按键的事件并清空记录。
    ///
    /// 顺序：先松普通键，再松修饰键，最后松鼠标按键（在 `position` 处）。
    /// 修饰键最后松开，避免残留的普通键与修饰键组合成快捷键。
    pub fn release_all(&mut self, position: Point) -> Vec<InputEvent> {
        let (modifiers, others): (Vec<_>, Vec<_>) = std::mem::take(&mut self.keys)
            .into_iter()
            .partition(|k| k.is_modifier());
        let keys = others
            .into_iter()
            .chain(modifiers)
            .map(|code| InputEvent::Key {
                code,
                action: KeyAction::Up,
            });
        let buttons =
            std::mem::take(&mut self.buttons)
                .into_iter()
                .map(|button| InputEvent::Button {
                    button,
                    pressed: false,
                    position,
                });
        keys.chain(buttons).collect()
    }
}
