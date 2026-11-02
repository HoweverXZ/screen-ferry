// 按键的显示名。键码是 USB HID 扩展用法码：高 16 位用法页（键盘页 0x07、消费者页 0x0C），低 16 位用法 ID，
// 与协议、与 Flutter 的 PhysicalKeyboardKey.usbHidUsage 一致。同一个键在 Mac 和 Windows 键盘上叫法不同，按系统取名。

import '../ipc/messages.dart';
import '../l10n/app_localizations.dart';

/// 键盘页上的键。
int keyboardKey(int usage) => 0x00070000 | usage;

/// 消费者页上的键（媒体键）。
int consumerKey(int usage) => 0x000C0000 | usage;

/// 一个键显示给用户的名字。[mac]：按 Mac 键盘上的叫法。不认识的键显示用法码。
String keyName(AppLocalizations l, int code, {required bool mac}) {
  final page = code >> 16;
  final usage = code & 0xFFFF;
  if (page == 0x0C) return _consumerName(l, usage) ?? l.keyMedia(_hex(usage));
  if (page != 0x07) return l.keyUnknown(_hex(code));
  if (usage >= 0x04 && usage <= 0x1D) return String.fromCharCode(0x41 + usage - 0x04);
  if (usage >= 0x1E && usage <= 0x26) return '${usage - 0x1D}';
  if (usage == 0x27) return '0';
  if (usage >= 0x3A && usage <= 0x45) return 'F${usage - 0x39}';
  if (usage >= 0x68 && usage <= 0x73) return 'F${usage - 0x68 + 13}';
  if (usage >= 0x59 && usage <= 0x61) return l.keyNumpad('${usage - 0x58}');
  return (mac ? _macName(l, usage) : null) ?? _keyName(l, usage) ?? l.keyUnknown(_hex(usage));
}

String _hex(int value) => value.toRadixString(16).toUpperCase();

/// 叫得出名字的所有键，添加映射时供搜索。
final List<int> knownKeys = [
  for (var u = 0x04; u <= 0x27; u++) keyboardKey(u),
  for (final u in _keyboardUsages) keyboardKey(u),
  for (var u = 0x3A; u <= 0x45; u++) keyboardKey(u),
  for (var u = 0x68; u <= 0x73; u++) keyboardKey(u),
  for (var u = 0x59; u <= 0x61; u++) keyboardKey(u),
  for (final u in _consumerUsages) consumerKey(u),
];

/// 添加映射时先列出来的常用目标键。0 表示不发送。
final List<int> commonTargets = [
  keyboardKey(0xE0), // Ctrl
  keyboardKey(0xE2), // Alt / ⌥
  keyboardKey(0xE3), // Win / ⌘
  keyboardKey(0xE1), // Shift
  keyboardKey(0x29), // Esc
  keyboardKey(0x4C), // Delete / ⌦
  keyboardKey(0x4A), // Home
  keyboardKey(0x4D), // End
  keyboardKey(0x4B), // Page Up
  keyboardKey(0x4E), // Page Down
  keyboardKey(0x46), // Print Screen
  keyboardKey(0x49), // Insert
  keyboardKey(0x65), // 菜单键
  consumerKey(0xE9), // 音量 +
  consumerKey(0xEA), // 音量 −
  consumerKey(0xE2), // 静音
  consumerKey(0xCD), // 播放/暂停
  0,
];

/// 修饰键显示名。
String modifierKeyName(ModifierKey key, {required bool mac}) => switch (key) {
  ModifierKey.gui => mac ? '⌘ Command' : 'Win',
  ModifierKey.ctrl => mac ? '⌃ Control' : 'Ctrl',
  ModifierKey.alt => mac ? '⌥ Option' : 'Alt',
  ModifierKey.capsLock => mac ? '⇪ Caps Lock' : 'Caps Lock',
};

/// 修饰键要变成的键的显示名。
String modifierTargetName(AppLocalizations l, ModifierTarget target, {required bool mac}) => switch (target) {
  ModifierTarget.ctrl => mac ? '⌃ Control' : 'Ctrl',
  ModifierTarget.shift => mac ? '⇧ Shift' : 'Shift',
  ModifierTarget.alt => mac ? '⌥ Option' : 'Alt',
  ModifierTarget.gui => mac ? '⌘ Command' : 'Win',
  ModifierTarget.capsLock => mac ? '⇪ Caps Lock' : 'Caps Lock',
  ModifierTarget.none => l.keyMapNothing,
};

/// 两边叫法相同、或 Windows 上的叫法。
String? _keyName(AppLocalizations l, int usage) => switch (usage) {
  0x2C => l.keySpace,
  0x54 => l.keyNumpad('/'),
  0x55 => l.keyNumpad('*'),
  0x56 => l.keyNumpad('-'),
  0x57 => l.keyNumpad('+'),
  0x58 => l.keyNumpad('Enter'),
  0x62 => l.keyNumpad('0'),
  0x63 => l.keyNumpad('.'),
  0x64 => l.keyIso,
  0x65 => l.keyMenu,
  0x67 => l.keyNumpad('='),
  0x7F => l.keyMute,
  0x80 => l.keyVolumeUp,
  0x81 => l.keyVolumeDown,
  0x90 => l.keyHangul,
  0x91 => l.keyHanja,
  0xE0 => l.keyLeft('Ctrl'),
  0xE1 => l.keyLeft('Shift'),
  0xE2 => l.keyLeft('Alt'),
  0xE3 => l.keyLeft('Win'),
  0xE4 => l.keyRight('Ctrl'),
  0xE5 => l.keyRight('Shift'),
  0xE6 => l.keyRight('Alt'),
  0xE7 => l.keyRight('Win'),
  _ => _keyboard[usage],
};

/// 叫法不随语言变的键。
const _keyboard = <int, String>{
  0x28: 'Enter',
  0x29: 'Esc',
  0x2A: 'Backspace',
  0x2B: 'Tab',
  0x2D: '-',
  0x2E: '=',
  0x2F: '[',
  0x30: ']',
  0x31: r'\',
  0x33: ';',
  0x34: "'",
  0x35: '`',
  0x36: ',',
  0x37: '.',
  0x38: '/',
  0x39: 'Caps Lock',
  0x46: 'Print Screen',
  0x47: 'Scroll Lock',
  0x48: 'Pause',
  0x49: 'Insert',
  0x4A: 'Home',
  0x4B: 'Page Up',
  0x4C: 'Delete',
  0x4D: 'End',
  0x4E: 'Page Down',
  0x4F: '→',
  0x50: '←',
  0x51: '↓',
  0x52: '↑',
  0x53: 'Num Lock',
  0x87: 'ろ',
  0x89: '¥',
};

/// 键盘页上叫得出名字的键（字母、数字、F 键和小键盘数字除外），按用法 ID 排。
const _keyboardUsages = [
  0x28, 0x29, 0x2A, 0x2B, 0x2C, 0x2D, 0x2E, 0x2F, 0x30, 0x31, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, //
  0x46, 0x47, 0x48, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x4F, 0x50, 0x51, 0x52, 0x53, 0x54, 0x55, 0x56, //
  0x57, 0x58, 0x62, 0x63, 0x64, 0x65, 0x67, 0x7F, 0x80, 0x81, 0x87, 0x89, 0x90, 0x91, //
  0xE0, 0xE1, 0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7,
];

/// Mac 键盘上叫法不同的键。
String? _macName(AppLocalizations l, int usage) => switch (usage) {
  0x28 => '↩ Return',
  0x2A => '⌫ Delete',
  0x39 => '⇪ Caps Lock',
  0x4C => l.keyForwardDelete,
  0xE0 => l.keyLeft('⌃'),
  0xE1 => l.keyLeft('⇧'),
  0xE2 => l.keyLeft('⌥'),
  0xE3 => l.keyLeft('⌘'),
  0xE4 => l.keyRight('⌃'),
  0xE5 => l.keyRight('⇧'),
  0xE6 => l.keyRight('⌥'),
  0xE7 => l.keyRight('⌘'),
  _ => null,
};

/// 消费者页的媒体键。
String? _consumerName(AppLocalizations l, int usage) => switch (usage) {
  0x6F => l.keyBrightnessUp,
  0x70 => l.keyBrightnessDown,
  0xB5 => l.keyNextTrack,
  0xB6 => l.keyPreviousTrack,
  0xB7 => l.keyStop,
  0xCD => l.keyPlayPause,
  0xE2 => l.keyMute,
  0xE9 => l.keyVolumeUp,
  0xEA => l.keyVolumeDown,
  _ => null,
};

const _consumerUsages = [0x6F, 0x70, 0xB5, 0xB6, 0xB7, 0xCD, 0xE2, 0xE9, 0xEA];
