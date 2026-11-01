//! 按键映射：服务端把本机的按键发给一台设备之前，按那台设备的设置换成对方的键（ADR 0009）。
//!
//! 一个键先看“其他按键映射”，再看修饰键的对应关系，其余原样发送。
//!
//! 打开了文本编辑快捷键、两台又不是同一种键盘时，把本机习惯的按法换成对方的按法，
//! 例如 Mac 控制 Windows 时 ⌘← 变成 Home。换的时候：
//! - 先在对方那边松开触发它的修饰键，再按目标键；修饰键先不按回去，等用户再按普通键时才按回去。
//!   这样单独松开 ⌘ 时对方什么也收不到，Windows 不会因为单独按了一下 Windows 键弹出开始菜单。
//! - 在 Windows 上单独松开 Alt 会激活菜单栏、单独松开 Windows 键会弹出开始菜单。规则本身要按 Ctrl 的，
//!   先按下 Ctrl 再松开它们；不用按 Ctrl 的，先轻点一下 Ctrl 再松开，两者都不再算“单独按了一下”。
//! - 每个键按下时发给对方的是什么都记下来，松开时照着发，中途换了设置也对得上。

use ferry_types::{DeviceSettings, KeyAction, KeyCode, ModifierMap, ModifierTarget};

const LEFT: KeyCode = KeyCode::keyboard(0x50);
const RIGHT: KeyCode = KeyCode::keyboard(0x4F);
const DOWN: KeyCode = KeyCode::keyboard(0x51);
const UP: KeyCode = KeyCode::keyboard(0x52);
const HOME: KeyCode = KeyCode::keyboard(0x4A);
const END: KeyCode = KeyCode::keyboard(0x4D);
const BACKSPACE: KeyCode = KeyCode::keyboard(0x2A);
const DELETE_FORWARD: KeyCode = KeyCode::keyboard(0x4C);

/// 参与快捷键判断的三类修饰键，左右都算。Shift 不在其中：按着就一直按着。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    Gui,
    Ctrl,
    Alt,
}

impl Class {
    fn of(code: KeyCode) -> Option<Self> {
        match code {
            KeyCode::LEFT_GUI | KeyCode::RIGHT_GUI => Some(Self::Gui),
            KeyCode::LEFT_CTRL | KeyCode::RIGHT_CTRL => Some(Self::Ctrl),
            KeyCode::LEFT_ALT | KeyCode::RIGHT_ALT => Some(Self::Alt),
            _ => None,
        }
    }
}

/// 一条快捷键转换规则：本机按着 `with` 那一类修饰键（`None` 表示三类都没按着）再按 `key`，
/// 对方收到 `out`。
struct Rule {
    with: Option<Class>,
    key: KeyCode,
    out: Out,
}

enum Out {
    /// 按着这个修饰键（可以没有）按这个键。
    Chord(Option<KeyCode>, KeyCode),
    /// 删到行首：Shift+Home 选中到行首，再按退格。
    DeleteToLineStart,
}

const fn chord(with: Option<Class>, key: KeyCode, modifier: Option<KeyCode>, to: KeyCode) -> Rule {
    Rule {
        with,
        key,
        out: Out::Chord(modifier, to),
    }
}

/// Mac 控制 Windows：Mac 的按法换成 Windows 的。
const MAC_TO_WINDOWS: [Rule; 9] = [
    chord(Some(Class::Gui), LEFT, None, HOME),
    chord(Some(Class::Gui), RIGHT, None, END),
    chord(Some(Class::Gui), UP, Some(KeyCode::LEFT_CTRL), HOME),
    chord(Some(Class::Gui), DOWN, Some(KeyCode::LEFT_CTRL), END),
    chord(Some(Class::Alt), LEFT, Some(KeyCode::LEFT_CTRL), LEFT),
    chord(Some(Class::Alt), RIGHT, Some(KeyCode::LEFT_CTRL), RIGHT),
    chord(
        Some(Class::Alt),
        BACKSPACE,
        Some(KeyCode::LEFT_CTRL),
        BACKSPACE,
    ),
    chord(
        Some(Class::Alt),
        DELETE_FORWARD,
        Some(KeyCode::LEFT_CTRL),
        DELETE_FORWARD,
    ),
    Rule {
        with: Some(Class::Gui),
        key: BACKSPACE,
        out: Out::DeleteToLineStart,
    },
];

/// Windows 控制 Mac：Windows 的按法换成 Mac 的。
const WINDOWS_TO_MAC: [Rule; 8] = [
    chord(None, HOME, Some(KeyCode::LEFT_GUI), LEFT),
    chord(None, END, Some(KeyCode::LEFT_GUI), RIGHT),
    chord(Some(Class::Ctrl), HOME, Some(KeyCode::LEFT_GUI), UP),
    chord(Some(Class::Ctrl), END, Some(KeyCode::LEFT_GUI), DOWN),
    chord(Some(Class::Ctrl), LEFT, Some(KeyCode::LEFT_ALT), LEFT),
    chord(Some(Class::Ctrl), RIGHT, Some(KeyCode::LEFT_ALT), RIGHT),
    chord(
        Some(Class::Ctrl),
        BACKSPACE,
        Some(KeyCode::LEFT_ALT),
        BACKSPACE,
    ),
    chord(
        Some(Class::Ctrl),
        DELETE_FORWARD,
        Some(KeyCode::LEFT_ALT),
        DELETE_FORWARD,
    ),
];

/// 本机按着的一个键。
#[derive(Clone, Copy, Debug)]
struct Held {
    local: KeyCode,
    /// 对方现在按着的、由它而来的键；没有发（屏蔽、被快捷键转换用掉、暂时松开了）为 `None`。
    sent: Option<KeyCode>,
    /// 为快捷键转换在对方那边暂时松开的修饰键：用户再按普通键之前要按回去。
    pending: Option<KeyCode>,
}

/// 正在进行的一次快捷键转换。
#[derive(Clone, Copy, Debug)]
struct Active {
    /// 本机触发它的键。
    key: KeyCode,
    /// 对方按着的目标键。
    out: KeyCode,
    /// 为它在对方按下的修饰键，结束时松开。
    modifier: Option<KeyCode>,
}

/// 一台设备的按键映射。服务端每个会话一份，光标离开那台设备时 [`reset`](Self::reset)。
#[derive(Clone, Debug)]
pub struct KeyMapper {
    /// 其他按键映射：本机键 → 对方键，`None` 表示不发送。
    extra: Vec<(KeyCode, Option<KeyCode>)>,
    modifiers: ModifierMap,
    /// 文本编辑快捷键的转换规则；没打开、或两台是同一种键盘时为空。
    rules: &'static [Rule],
    /// 对方是 Windows：单独松开 Alt、Windows 键要防着。
    remote_windows: bool,
    held: Vec<Held>,
    active: Option<Active>,
}

impl std::fmt::Debug for Rule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}+{:?}", self.with, self.key)
    }
}

impl KeyMapper {
    /// `local_command`：本机键盘以 Command 为主（Mac）；`remote_command`：对方以 Command 为主。
    pub fn new(settings: &DeviceSettings, local_command: bool, remote_command: bool) -> Self {
        let differ = local_command != remote_command;
        let mut extra: Vec<(KeyCode, Option<KeyCode>)> = Vec::new();
        for m in &settings.keys {
            let from = KeyCode::from_raw(m.from);
            // 同一个键只看第一条。
            if extra.iter().any(|(f, _)| *f == from) {
                continue;
            }
            let to = (m.to != 0).then(|| KeyCode::from_raw(m.to));
            extra.push((from, to));
        }
        let rules: &'static [Rule] = match (settings.text_shortcuts && differ, local_command) {
            (false, _) => &[],
            (true, true) => &MAC_TO_WINDOWS,
            (true, false) => &WINDOWS_TO_MAC,
        };
        Self {
            extra,
            modifiers: settings.modifier_map(differ),
            rules,
            remote_windows: !remote_command,
            held: Vec::new(),
            active: None,
        }
    }

    /// 本机按下或松开一个键（本机的物理键码），返回依次要发给对方的按键（对方的键码）。
    /// 屏蔽的键返回空。
    pub fn key(&mut self, code: KeyCode, action: KeyAction) -> Vec<(KeyCode, KeyAction)> {
        let mut out = Vec::new();
        match action {
            KeyAction::Down => self.down(code, &mut out),
            KeyAction::Repeat => self.repeat(code, &mut out),
            KeyAction::Up => self.up(code, &mut out),
        }
        out
    }

    /// 光标离开对方、会话结束时调用：忘掉按着的键和正在进行的转换。对方收到 Leave 会自己松开所有键。
    pub fn reset(&mut self) {
        self.held.clear();
        self.active = None;
    }

    /// 一个键按设置换成对方的哪个键；`None` 表示不发送。
    fn map(&self, code: KeyCode) -> Option<KeyCode> {
        if let Some(&(_, to)) = self.extra.iter().find(|(from, _)| *from == code) {
            return to;
        }
        let m = self.modifiers;
        let (target, right) = match code {
            KeyCode::LEFT_GUI => (m.gui, false),
            KeyCode::RIGHT_GUI => (m.gui, true),
            KeyCode::LEFT_CTRL => (m.ctrl, false),
            KeyCode::RIGHT_CTRL => (m.ctrl, true),
            KeyCode::LEFT_ALT => (m.alt, false),
            KeyCode::RIGHT_ALT => (m.alt, true),
            KeyCode::CAPS_LOCK => (m.caps_lock, false),
            _ => return Some(code),
        };
        modifier_code(target, right)
    }

    fn down(&mut self, code: KeyCode, out: &mut Vec<(KeyCode, KeyAction)>) {
        if self.active.is_some_and(|a| a.key == code) {
            // 有的系统按住时发的是 Down 而不是 Repeat。
            return self.repeat(code, out);
        }
        // 同一个键又按了一次（没收到松开）：先按松开处理，免得对方那边多按着一个。
        if self.held.iter().any(|h| h.local == code) {
            self.up(code, out);
        }
        if let Some(rule) = self.rule_for(code) {
            self.finish(out);
            self.start(code, rule, out);
            return;
        }
        if !is_modifier(code) {
            // 普通键：先结束正在进行的转换，再把暂时松开的修饰键按回去，组合键照常生效。
            self.finish(out);
            for h in &mut self.held {
                if let Some(t) = h.pending.take() {
                    h.sent = Some(t);
                    out.push((t, KeyAction::Down));
                }
            }
        }
        let sent = self.map(code);
        if let Some(t) = sent {
            out.push((t, KeyAction::Down));
        }
        self.held.push(Held {
            local: code,
            sent,
            pending: None,
        });
    }

    fn repeat(&mut self, code: KeyCode, out: &mut Vec<(KeyCode, KeyAction)>) {
        if let Some(active) = self.active.filter(|a| a.key == code) {
            out.push((active.out, KeyAction::Repeat));
            return;
        }
        let sent = match self.held.iter().find(|h| h.local == code) {
            Some(h) => h.sent,
            // 光标过来之前就按着的键。
            None => self.map(code),
        };
        if let Some(t) = sent {
            out.push((t, KeyAction::Repeat));
        }
    }

    fn up(&mut self, code: KeyCode, out: &mut Vec<(KeyCode, KeyAction)>) {
        if self.active.is_some_and(|a| a.key == code) {
            self.finish(out);
        }
        match self.held.iter().position(|h| h.local == code) {
            Some(i) => {
                let h = self.held.remove(i);
                // 暂时松开了的修饰键在对方那边已经是松开的。
                if let Some(t) = h.sent {
                    out.push((t, KeyAction::Up));
                }
            }
            // 光标过来之前就按着的键：照现在的映射松开。
            None => {
                if let Some(t) = self.map(code) {
                    out.push((t, KeyAction::Up));
                }
            }
        }
    }

    /// 按现在本机按着的修饰键，`code` 触发哪条规则。
    fn rule_for(&self, code: KeyCode) -> Option<&'static Rule> {
        let rules = self.rules;
        let held = |class| self.held.iter().any(|h| Class::of(h.local) == Some(class));
        let classes = [Class::Gui, Class::Ctrl, Class::Alt];
        rules
            .iter()
            .find(|r| r.key == code && classes.iter().all(|&c| held(c) == (r.with == Some(c))))
    }

    /// 开始一次转换。
    fn start(&mut self, code: KeyCode, rule: &Rule, out: &mut Vec<(KeyCode, KeyAction)>) {
        let wanted = match rule.out {
            Out::Chord(modifier, _) => modifier,
            Out::DeleteToLineStart => None,
        };
        // 触发它的那一类修饰键，现在在对方按着的。规则要按的修饰键正好已经按着，就留着用。
        let mut keep = None;
        let mut release = Vec::new();
        for h in &self.held {
            let (Some(class), Some(t)) = (Class::of(h.local), h.sent) else {
                continue;
            };
            if Some(class) != rule.with {
                continue;
            }
            if Some(t) == wanted && keep.is_none() {
                keep = Some(t);
            } else {
                release.push(t);
            }
        }
        let modifier = wanted.filter(|_| keep.is_none());
        if let Some(m) = modifier {
            out.push((m, KeyAction::Down));
        } else if self.remote_windows
            && release.iter().any(|&t| is_alt_or_gui(t))
            && !self.remote_down(is_ctrl)
        {
            out.push((KeyCode::LEFT_CTRL, KeyAction::Down));
            out.push((KeyCode::LEFT_CTRL, KeyAction::Up));
        }
        for h in &mut self.held {
            if let Some(t) = h.sent
                && release.contains(&t)
            {
                h.sent = None;
                h.pending = Some(t);
                out.push((t, KeyAction::Up));
            }
        }
        let target = match rule.out {
            Out::Chord(_, key) => key,
            Out::DeleteToLineStart => {
                let shift = !self.remote_down(is_shift);
                if shift {
                    out.push((KeyCode::LEFT_SHIFT, KeyAction::Down));
                }
                out.push((HOME, KeyAction::Down));
                out.push((HOME, KeyAction::Up));
                if shift {
                    out.push((KeyCode::LEFT_SHIFT, KeyAction::Up));
                }
                BACKSPACE
            }
        };
        out.push((target, KeyAction::Down));
        self.active = Some(Active {
            key: code,
            out: target,
            modifier,
        });
        // 触发键本身不发：它松开时只结束转换。
        self.held.push(Held {
            local: code,
            sent: None,
            pending: None,
        });
    }

    /// 结束正在进行的转换：松开目标键和为它按下的修饰键。用户的修饰键等再按普通键时才按回去。
    fn finish(&mut self, out: &mut Vec<(KeyCode, KeyAction)>) {
        if let Some(active) = self.active.take() {
            out.push((active.out, KeyAction::Up));
            if let Some(m) = active.modifier {
                out.push((m, KeyAction::Up));
            }
        }
    }

    /// 对方现在是否按着满足 `pred` 的键。
    fn remote_down(&self, pred: fn(KeyCode) -> bool) -> bool {
        self.held.iter().filter_map(|h| h.sent).any(pred)
            || self.active.and_then(|a| a.modifier).is_some_and(pred)
    }
}

fn modifier_code(target: ModifierTarget, right: bool) -> Option<KeyCode> {
    let pick = |left, r| Some(if right { r } else { left });
    match target {
        ModifierTarget::Ctrl => pick(KeyCode::LEFT_CTRL, KeyCode::RIGHT_CTRL),
        ModifierTarget::Shift => pick(KeyCode::LEFT_SHIFT, KeyCode::RIGHT_SHIFT),
        ModifierTarget::Alt => pick(KeyCode::LEFT_ALT, KeyCode::RIGHT_ALT),
        ModifierTarget::Gui => pick(KeyCode::LEFT_GUI, KeyCode::RIGHT_GUI),
        ModifierTarget::CapsLock => Some(KeyCode::CAPS_LOCK),
        ModifierTarget::None => None,
    }
}

/// 键盘页上的八个修饰键（左右 Ctrl、Shift、Alt、GUI）。
fn is_modifier(code: KeyCode) -> bool {
    (KeyCode::LEFT_CTRL.raw()..=KeyCode::RIGHT_GUI.raw()).contains(&code.raw())
}

fn is_ctrl(code: KeyCode) -> bool {
    matches!(code, KeyCode::LEFT_CTRL | KeyCode::RIGHT_CTRL)
}

fn is_shift(code: KeyCode) -> bool {
    matches!(code, KeyCode::LEFT_SHIFT | KeyCode::RIGHT_SHIFT)
}

fn is_alt_or_gui(code: KeyCode) -> bool {
    matches!(
        code,
        KeyCode::LEFT_ALT | KeyCode::RIGHT_ALT | KeyCode::LEFT_GUI | KeyCode::RIGHT_GUI
    )
}
