// 填 DPI 的小输入框：右边带着“DPI”。回车或离开时提交，超出范围的收进范围内，不是数字的恢复原值。

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../theme/tokens.dart';

class DpiField extends StatefulWidget {
  const DpiField({
    super.key,
    required this.value,
    required this.onChanged,
    required this.label,
    this.min = 100,
    this.max = 12800,
    this.highlight = false,
  });

  final int value;

  /// 为 null 时只读。
  final ValueChanged<int>? onChanged;

  /// 读屏用的名字，例如“光标速度”。
  final String label;
  final int min;
  final int max;

  /// 和缺省值不一样时用强调色描边。
  final bool highlight;

  @override
  State<DpiField> createState() => _DpiFieldState();
}

class _DpiFieldState extends State<DpiField> {
  late final _controller = TextEditingController(text: '${widget.value}');
  final _focus = FocusNode();

  @override
  void initState() {
    super.initState();
    _focus.addListener(() {
      if (!_focus.hasFocus) _commit();
    });
  }

  @override
  void didUpdateWidget(DpiField old) {
    super.didUpdateWidget(old);
    // 输入时不打断；别处改了（拖动条、还原）时跟上。
    if (!_focus.hasFocus && old.value != widget.value) _controller.text = '${widget.value}';
  }

  @override
  void dispose() {
    _controller.dispose();
    _focus.dispose();
    super.dispose();
  }

  void _commit() {
    final parsed = int.tryParse(_controller.text.trim());
    if (parsed == null) {
      _controller.text = '${widget.value}';
      return;
    }
    final v = parsed.clamp(widget.min, widget.max);
    _controller.text = '$v';
    if (v != widget.value) widget.onChanged?.call(v);
  }

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    return Container(
      height: 38,
      padding: const EdgeInsets.only(left: 6, right: 12),
      decoration: ShapeDecoration(
        color: c.card2,
        shape: continuous(12, side: BorderSide(color: widget.highlight ? c.accent : Colors.transparent, width: 1.5)),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          SizedBox(
            width: 64,
            child: Semantics(
              label: widget.label,
              child: TextField(
                controller: _controller,
                focusNode: _focus,
                enabled: widget.onChanged != null,
                textAlign: TextAlign.right,
                keyboardType: TextInputType.number,
                inputFormatters: [FilteringTextInputFormatter.digitsOnly, LengthLimitingTextInputFormatter(5)],
                onSubmitted: (_) => _commit(),
                style: const TextStyle(fontSize: 16, fontWeight: FontWeight.w700, fontFeatures: tabular),
                decoration: const InputDecoration(
                  isDense: true,
                  border: InputBorder.none,
                  contentPadding: EdgeInsets.symmetric(vertical: 8),
                ),
              ),
            ),
          ),
          const SizedBox(width: 6),
          Text(
            'DPI',
            style: TextStyle(fontSize: 13, fontWeight: FontWeight.w700, color: c.muted),
          ),
        ],
      ),
    );
  }
}
