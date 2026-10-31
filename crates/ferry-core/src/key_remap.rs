//! 按键重映射：发送前把某些物理键换成另一个键，例如把 Mac 的 Command 当作 Windows 的 Ctrl。

use ferry_types::KeyCode;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeyRemap {
    pairs: Vec<(KeyCode, KeyCode)>,
}

impl KeyRemap {
    /// 不做任何映射。
    pub fn none() -> Self {
        Self::default()
    }

    /// Command（GUI）与 Control 互换，左右各自对应。
    /// 这样在 Windows 上按 ⌘C、⌘V 就是复制、粘贴，与在 Mac 上的习惯一致。
    pub fn swap_command_control() -> Self {
        Self {
            pairs: vec![
                (KeyCode::LEFT_GUI, KeyCode::LEFT_CTRL),
                (KeyCode::LEFT_CTRL, KeyCode::LEFT_GUI),
                (KeyCode::RIGHT_GUI, KeyCode::RIGHT_CTRL),
                (KeyCode::RIGHT_CTRL, KeyCode::RIGHT_GUI),
            ],
        }
    }

    pub fn apply(&self, code: KeyCode) -> KeyCode {
        self.pairs
            .iter()
            .find(|(from, _)| *from == code)
            .map_or(code, |&(_, to)| to)
    }
}
