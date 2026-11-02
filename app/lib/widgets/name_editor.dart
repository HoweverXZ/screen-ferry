// 原地改名：平时显示名字和一个 ✎ 按钮，点了换成输入框和“保存”“取消”。回车保存，Esc 取消。

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../l10n/l10n.dart';
import '../theme/tokens.dart';
import 'surface.dart';

class NameEditor extends StatefulWidget {
  const NameEditor({
    super.key,
    required this.name,
    required this.onSave,
    required this.label,
    this.placeholder = '',
    this.hint,
    this.enabled = true,
    this.style = const TextStyle(fontSize: 22, fontWeight: FontWeight.w700),
  });

  /// 现在显示的名字。
  final String name;

  /// 保存输入的名字（已去掉首尾空白，可能为空）。
  final ValueChanged<String> onSave;

  /// 输入框上方的说明，例如“在本机上叫它”，也用作读屏标签。
  final String label;

  /// 输入框留空时显示的字。
  final String placeholder;

  /// 输入框下方的提示。
  final String? hint;
  final bool enabled;
  final TextStyle style;

  @override
  State<NameEditor> createState() => _NameEditorState();
}

class _NameEditorState extends State<NameEditor> {
  final _controller = TextEditingController();
  final _focus = FocusNode();
  bool _editing = false;

  @override
  void dispose() {
    _controller.dispose();
    _focus.dispose();
    super.dispose();
  }

  void _start() {
    _controller.text = widget.name;
    _controller.selection = TextSelection(baseOffset: 0, extentOffset: widget.name.length);
    setState(() => _editing = true);
    WidgetsBinding.instance.addPostFrameCallback((_) => _focus.requestFocus());
  }

  void _save() {
    widget.onSave(_controller.text.trim());
    setState(() => _editing = false);
  }

  void _cancel() => setState(() => _editing = false);

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    if (!_editing) {
      return Row(
        children: [
          Flexible(
            child: Text(widget.name, maxLines: 1, overflow: TextOverflow.ellipsis, style: widget.style),
          ),
          const SizedBox(width: 6),
          SfIconButton(
            icon: Icons.edit_outlined,
            tooltip: context.l10n.nameEditorRename,
            size: 32,
            onPressed: widget.enabled ? _start : null,
          ),
        ],
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(
          widget.label,
          style: TextStyle(fontSize: 13, fontWeight: FontWeight.w600, color: c.muted),
        ),
        const SizedBox(height: 6),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: [
            ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 320, minWidth: 160),
              child: CallbackShortcuts(
                bindings: {const SingleActivator(LogicalKeyboardKey.escape): _cancel},
                child: TextField(
                  controller: _controller,
                  focusNode: _focus,
                  maxLength: 40,
                  onSubmitted: (_) => _save(),
                  style: widget.style.copyWith(fontSize: 17),
                  decoration: InputDecoration(
                    isDense: true,
                    counterText: '',
                    hintText: widget.placeholder,
                    hintStyle: TextStyle(color: c.muted, fontWeight: FontWeight.w500),
                    filled: true,
                    fillColor: c.card,
                    contentPadding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
                    border: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(12),
                      borderSide: BorderSide(color: c.accent, width: 2),
                    ),
                    enabledBorder: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(12),
                      borderSide: BorderSide(color: c.accent, width: 2),
                    ),
                    focusedBorder: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(12),
                      borderSide: BorderSide(color: c.accent, width: 2),
                    ),
                  ),
                ),
              ),
            ),
            SmallButton(label: context.l10n.commonSave, tone: PillTone.accent, onPressed: _save),
            SmallButton(label: context.l10n.commonCancel, onPressed: _cancel),
          ],
        ),
        if (widget.hint != null) ...[
          const SizedBox(height: 8),
          Text(widget.hint!, style: TextStyle(fontSize: 13, color: c.muted, height: 1.5)),
        ],
      ],
    );
  }
}
