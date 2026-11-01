// 按键的显示名。键码是 USB HID 扩展用法码：高 16 位用法页（键盘页 0x07、消费者页 0x0C），低 16 位用法 ID，
// 与协议、与 Flutter 的 PhysicalKeyboardKey.usbHidUsage 一致。同一个键在 Mac 和 Windows 键盘上叫法不同，按系统取名。

import '../ipc/messages.dart';

/// 键盘页上的键。
int keyboardKey(int usage) => 0x00070000 | usage;

/// 消费者页上的键（媒体键）。
int consumerKey(int usage) => 0x000C0000 | usage;

/// 一个键显示给用户的名字。[mac]：按 Mac 键盘上的叫法。不认识的键显示用法码。
String keyName(int code, {required bool mac}) {
  final page = code >> 16;
  final usage = code & 0xFFFF;
  if (page == 0x0C) {
    return _consumer[usage] ?? '媒体键 0x${usage.toRadixString(16).toUpperCase()}';
  }
  if (page != 0x07) {
    return '按键 0x${code.toRadixString(16).toUpperCase()}';
  }
  if (usage >= 0x04 && usage <= 0x1D) return String.fromCharCode(0x41 + usage - 0x04);
  if (usage >= 0x1E && usage <= 0x26) return '${usage - 0x1D}';
  if (usage == 0x27) return '0';
  if (usage >= 0x3A && usage <= 0x45) return 'F${usage - 0x39}';
  if (usage >= 0x68 && usage <= 0x73) return 'F${usage - 0x68 + 13}';
  if (usage >= 0x59 && usage <= 0x61) return '小键盘 ${usage - 0x58}';
  final named = (mac ? _macKeyboard[usage] : null) ?? _keyboard[usage];
  return named ?? '按键 0x${usage.toRadixString(16).toUpperCase()}';
}

/// 叫得出名字的所有键，添加映射时供搜索。
final List<int> knownKeys = [
  for (var u = 0x04; u <= 0x27; u++) keyboardKey(u),
  for (final u in _keyboard.keys) keyboardKey(u),
  for (var u = 0x3A; u <= 0x45; u++) keyboardKey(u),
  for (var u = 0x68; u <= 0x73; u++) keyboardKey(u),
  for (var u = 0x59; u <= 0x61; u++) keyboardKey(u),
  for (final u in _consumer.keys) consumerKey(u),
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
String modifierTargetName(ModifierTarget target, {required bool mac}) => switch (target) {
  ModifierTarget.ctrl => mac ? '⌃ Control' : 'Ctrl',
  ModifierTarget.shift => mac ? '⇧ Shift' : 'Shift',
  ModifierTarget.alt => mac ? '⌥ Option' : 'Alt',
  ModifierTarget.gui => mac ? '⌘ Command' : 'Win',
  ModifierTarget.capsLock => mac ? '⇪ Caps Lock' : 'Caps Lock',
  ModifierTarget.none => '不发送',
};

/// 两边叫法相同、或 Windows 上的叫法。
const _keyboard = <int, String>{
  0x28: 'Enter',
  0x29: 'Esc',
  0x2A: 'Backspace',
  0x2B: 'Tab',
  0x2C: '空格',
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
  0x54: '小键盘 /',
  0x55: '小键盘 *',
  0x56: '小键盘 -',
  0x57: '小键盘 +',
  0x58: '小键盘 Enter',
  0x62: '小键盘 0',
  0x63: '小键盘 .',
  0x64: r'\（ISO）',
  0x65: '菜单键',
  0x67: '小键盘 =',
  0x7F: '静音',
  0x80: '音量 +',
  0x81: '音量 −',
  0x87: 'ろ',
  0x89: '¥',
  0x90: '韩/英、かな',
  0x91: '汉字、英数',
  0xE0: '左 Ctrl',
  0xE1: '左 Shift',
  0xE2: '左 Alt',
  0xE3: '左 Win',
  0xE4: '右 Ctrl',
  0xE5: '右 Shift',
  0xE6: '右 Alt',
  0xE7: '右 Win',
};

/// Mac 键盘上叫法不同的键。
const _macKeyboard = <int, String>{
  0x28: '↩ Return',
  0x2A: '⌫ Delete',
  0x39: '⇪ Caps Lock',
  0x4C: '⌦ 向前删除',
  0xE0: '左 ⌃',
  0xE1: '左 ⇧',
  0xE2: '左 ⌥',
  0xE3: '左 ⌘',
  0xE4: '右 ⌃',
  0xE5: '右 ⇧',
  0xE6: '右 ⌥',
  0xE7: '右 ⌘',
};

/// 消费者页的媒体键。
const _consumer = <int, String>{
  0x6F: '亮度 +',
  0x70: '亮度 −',
  0xB5: '下一首',
  0xB6: '上一首',
  0xB7: '停止',
  0xCD: '播放/暂停',
  0xE2: '静音',
  0xE9: '音量 +',
  0xEA: '音量 −',
};
