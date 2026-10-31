import 'dart:async';

import 'package:flutter/material.dart';

import '../state/app_state.dart';
import '../theme/tokens.dart';

/// 底部居中的胶囊提示，2 秒后自动收起。
class ToastHost extends StatefulWidget {
  const ToastHost({super.key, required this.messages});

  final ValueNotifier<ToastMessage?> messages;

  @override
  State<ToastHost> createState() => _ToastHostState();
}

class _ToastHostState extends State<ToastHost> {
  String _text = '';
  bool _visible = false;
  Timer? _hide;

  @override
  void initState() {
    super.initState();
    widget.messages.addListener(_show);
  }

  void _show() {
    final m = widget.messages.value;
    if (m == null) return;
    setState(() {
      _text = m.text;
      _visible = true;
    });
    _hide?.cancel();
    _hide = Timer(const Duration(seconds: 2), () {
      if (mounted) setState(() => _visible = false);
    });
  }

  @override
  void dispose() {
    widget.messages.removeListener(_show);
    _hide?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    return IgnorePointer(
      child: AnimatedSlide(
        offset: _visible ? Offset.zero : const Offset(0, 0.6),
        duration: Motion.medium,
        curve: Motion.spring,
        child: AnimatedOpacity(
          opacity: _visible ? 1 : 0,
          duration: Motion.fast,
          child: Semantics(
            liveRegion: true,
            child: Container(
              padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 12),
              decoration: ShapeDecoration(color: c.fg, shape: const StadiumBorder()),
              child: Text(
                _text,
                style: TextStyle(color: c.card, fontWeight: FontWeight.w600, fontSize: 14),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
