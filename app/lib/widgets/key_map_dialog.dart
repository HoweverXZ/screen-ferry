// 添加按键映射：先在本机键盘上按一个键录下来，再选对方收到什么（或者不发送）。
// 录键用键盘事件的物理键 `usbHidUsage`，就是协议用的 USB HID 扩展用法码。

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../ipc/messages.dart';
import '../state/key_names.dart';
import '../theme/tokens.dart';
import 'pressable.dart';
import 'surface.dart';

/// 弹出对话框。[localMac]、[peerMac]：两边是不是 Mac，决定键名的叫法。添加时返回新的映射，取消时返回 null。
Future<KeyMapping?> showKeyMapDialog(
  BuildContext context, {
  required String peerName,
  required bool localMac,
  required bool peerMac,
}) {
  return showGeneralDialog<KeyMapping>(
    context: context,
    barrierDismissible: true,
    barrierLabel: '添加按键映射',
    barrierColor: Colors.black.withValues(alpha: 0.34),
    transitionDuration: const Duration(milliseconds: 320),
    pageBuilder: (context, _, _) => KeyMapDialog(peerName: peerName, localMac: localMac, peerMac: peerMac),
    transitionBuilder: (context, animation, _, child) {
      final curved = CurvedAnimation(parent: animation, curve: Motion.spring, reverseCurve: Curves.easeIn);
      return FadeTransition(
        opacity: CurvedAnimation(parent: animation, curve: Curves.easeOut),
        child: ScaleTransition(scale: Tween(begin: 0.94, end: 1.0).animate(curved), child: child),
      );
    },
  );
}

class KeyMapDialog extends StatefulWidget {
  const KeyMapDialog({super.key, required this.peerName, required this.localMac, required this.peerMac});

  final String peerName;
  final bool localMac;
  final bool peerMac;

  @override
  State<KeyMapDialog> createState() => _KeyMapDialogState();
}

class _KeyMapDialogState extends State<KeyMapDialog> {
  final _recorder = FocusNode(debugLabel: '录键');
  final _search = TextEditingController();

  /// 正在等用户按键。
  bool _recording = true;

  /// 录下的本机键。
  int? _from;

  /// 选中的目标键；0 表示不发送。
  int? _to;

  @override
  void initState() {
    super.initState();
    _search.addListener(() => setState(() {}));
    // 点了别处（例如搜索框），就不再等着录键。
    _recorder.addListener(() {
      if (!_recorder.hasFocus && _recording) setState(() => _recording = false);
    });
    WidgetsBinding.instance.addPostFrameCallback((_) => _recorder.requestFocus());
  }

  @override
  void dispose() {
    _recorder.dispose();
    _search.dispose();
    super.dispose();
  }

  void _record() {
    setState(() => _recording = true);
    _recorder.requestFocus();
  }

  KeyEventResult _onKey(FocusNode _, KeyEvent event) {
    if (!_recording || event is! KeyDownEvent) return KeyEventResult.ignored;
    setState(() {
      _recording = false;
      // Esc 是取消录制，不录它。
      if (event.logicalKey != LogicalKeyboardKey.escape) _from = event.physicalKey.usbHidUsage;
    });
    return KeyEventResult.handled;
  }

  String _local(int code) => keyName(code, mac: widget.localMac);
  String _target(int code) => code == 0 ? '不发送' : keyName(code, mac: widget.peerMac);

  /// 网格里列出的目标键：没输入搜索时是常用键，否则是名字里含着输入的键。
  List<int> get _choices {
    final q = _search.text.trim().toLowerCase();
    if (q.isEmpty) return commonTargets;
    return [
      for (final code in [...knownKeys, 0])
        if (_target(code).toLowerCase().contains(q)) code,
    ].take(30).toList();
  }

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    final from = _from;
    final to = _to;
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Material(
          color: c.card,
          shape: continuous(28),
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 560),
            child: SingleChildScrollView(
              padding: const EdgeInsets.fromLTRB(26, 24, 26, 24),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                mainAxisSize: MainAxisSize.min,
                children: [
                  Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Semantics(
                              header: true,
                              child: const Text('添加按键映射', style: TextStyle(fontSize: 20, fontWeight: FontWeight.w700)),
                            ),
                            const SizedBox(height: 4),
                            Text('只对 ${widget.peerName} 生效', style: TextStyle(fontSize: 14, color: c.muted)),
                          ],
                        ),
                      ),
                      SfIconButton(
                        icon: Icons.close_rounded,
                        tooltip: '关闭',
                        onPressed: () => Navigator.of(context).pop(),
                      ),
                    ],
                  ),
                  const SizedBox(height: 18),
                  _label(c, '① 本机按哪个键'),
                  const SizedBox(height: 8),
                  Focus(
                    focusNode: _recorder,
                    onKeyEvent: _onKey,
                    child: Pressable(
                      onTap: _record,
                      color: _recording ? c.accentSoft : c.card2,
                      radius: 16,
                      pressScale: 0.99,
                      semanticLabel: _recording ? '正在录制，按一个键' : '重新录制',
                      padding: const EdgeInsets.symmetric(horizontal: 16),
                      child: SizedBox(
                        height: 60,
                        child: Row(
                          children: [
                            if (_recording) ...[
                              Container(
                                width: 10,
                                height: 10,
                                decoration: BoxDecoration(color: c.accent, shape: BoxShape.circle),
                              ),
                              const SizedBox(width: 12),
                              const Expanded(
                                child: Text('请在键盘上按一个键…', style: TextStyle(fontSize: 15, fontWeight: FontWeight.w600)),
                              ),
                              Text('Esc 取消', style: TextStyle(fontSize: 13, color: c.muted)),
                            ] else if (from != null) ...[
                              KeyCap(_local(from)),
                              const SizedBox(width: 10),
                              Text('已录下', style: TextStyle(fontSize: 14, color: c.muted)),
                              const Spacer(),
                              Text(
                                '重新录制',
                                style: TextStyle(fontSize: 14, fontWeight: FontWeight.w700, color: c.accent),
                              ),
                            ] else ...[
                              Expanded(
                                child: Text('点这里，再按一个键', style: TextStyle(fontSize: 15, color: c.muted)),
                              ),
                            ],
                          ],
                        ),
                      ),
                    ),
                  ),
                  const SizedBox(height: 6),
                  Text('也能录单个修饰键，左右分开算。', style: TextStyle(fontSize: 13, color: c.muted)),
                  const SizedBox(height: 18),
                  _label(c, '② ${widget.peerName} 收到什么'),
                  const SizedBox(height: 8),
                  TextField(
                    controller: _search,
                    decoration: InputDecoration(
                      isDense: true,
                      prefixIcon: Icon(Icons.search_rounded, size: 20, color: c.muted),
                      hintText: '搜索按键，例如 Home、F5、音量',
                      hintStyle: TextStyle(color: c.muted),
                      filled: true,
                      fillColor: c.card2,
                      contentPadding: const EdgeInsets.symmetric(vertical: 12),
                      border: OutlineInputBorder(borderRadius: BorderRadius.circular(14), borderSide: BorderSide.none),
                    ),
                  ),
                  const SizedBox(height: 12),
                  Wrap(
                    spacing: 8,
                    runSpacing: 8,
                    children: [
                      for (final code in _choices)
                        _Choice(label: _target(code), selected: to == code, onTap: () => setState(() => _to = code)),
                    ],
                  ),
                  if (from != null && to != null) ...[
                    const SizedBox(height: 16),
                    Container(
                      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
                      decoration: ShapeDecoration(color: c.accentSoft, shape: continuous(16)),
                      child: Wrap(
                        spacing: 10,
                        runSpacing: 8,
                        crossAxisAlignment: WrapCrossAlignment.center,
                        children: [
                          const Text('按', style: TextStyle(fontWeight: FontWeight.w600)),
                          KeyCap(_local(from)),
                          const Icon(Icons.arrow_forward_rounded, size: 18),
                          if (to == 0)
                            Text('${widget.peerName} 什么也收不到', style: const TextStyle(fontWeight: FontWeight.w600))
                          else ...[
                            Text('${widget.peerName} 收到', style: const TextStyle(fontWeight: FontWeight.w600)),
                            KeyCap(_target(to)),
                          ],
                        ],
                      ),
                    ),
                  ],
                  const SizedBox(height: 20),
                  Row(
                    mainAxisAlignment: MainAxisAlignment.end,
                    children: [
                      PillButton(label: '取消', soft: true, onPressed: () => Navigator.of(context).pop()),
                      const SizedBox(width: 10),
                      PillButton(
                        label: '添加',
                        primary: true,
                        onPressed: from != null && to != null
                            ? () => Navigator.of(context).pop(KeyMapping(from, to))
                            : null,
                      ),
                    ],
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }

  Widget _label(SfColors c, String text) => Text(
    text,
    style: TextStyle(fontSize: 14, fontWeight: FontWeight.w700, color: c.muted),
  );
}

/// 网格里的一个目标键。
class _Choice extends StatelessWidget {
  const _Choice({required this.label, required this.selected, required this.onTap});

  final String label;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    return Pressable(
      onTap: onTap,
      selected: selected,
      semanticLabel: label,
      color: selected ? c.accentSoft : c.card,
      radius: 12,
      pressScale: 0.96,
      child: AnimatedContainer(
        duration: Motion.fast,
        height: 36,
        padding: const EdgeInsets.symmetric(horizontal: 12),
        alignment: Alignment.center,
        decoration: ShapeDecoration(
          shape: continuous(12, side: BorderSide(color: selected ? c.accent : c.line, width: 1.5)),
        ),
        child: Text(
          label,
          style: TextStyle(fontSize: 14, fontWeight: FontWeight.w600, color: selected ? c.accent : c.fg),
        ),
      ),
    );
  }
}
