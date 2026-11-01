// 设备卡片上的设置摘要：和缺省设置不一样的项各一个小标签。

import '../ipc/messages.dart';

/// 和缺省设置不一样的项。全是缺省值时只有“默认设置”。
List<String> deviceSummary(DeviceSettings d) {
  final items = [
    if (d.pointerDpi != 0) '光标 ${d.pointerDpi} DPI',
    if (d.reverseVertical) '垂直滚动反向',
    if (d.reverseHorizontal) '水平滚动反向',
    if (d.scrollSpeed == ScrollSpeed.slow) '滚动慢',
    if (d.scrollSpeed == ScrollSpeed.fast) '滚动快',
    if (d.modifiers == ModifierPreset.plain) '按键原样',
    if (d.modifiers == ModifierPreset.custom) '自定义修饰键',
    if (d.keys.isNotEmpty) '${d.keys.length} 个按键映射',
    if (d.textShortcuts) '文本快捷键',
    if (d.edgeDelayMs > 0) '防误切',
    if (!d.clipboard) '不同步剪贴板',
    if (!d.files) '不能拖文件',
  ];
  return items.isEmpty ? const ['默认设置'] : items;
}
