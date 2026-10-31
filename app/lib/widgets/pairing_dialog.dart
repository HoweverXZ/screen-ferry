// 配对弹窗：6 位配对码依次弹出，倒计时结束自动拒绝，确认后显示成功动画。

import 'package:flutter/material.dart';

import '../state/app_state.dart';
import '../theme/tokens.dart';
import 'surface.dart';

const _timeout = Duration(seconds: 120);

/// 用户确认返回 `true`，拒绝或超时返回 `false`。
Future<bool> showPairingDialog(BuildContext context, {required Device device, required String code}) async {
  final result = await showGeneralDialog<bool>(
    context: context,
    barrierDismissible: false,
    barrierLabel: '配对',
    barrierColor: Colors.black.withValues(alpha: 0.34),
    transitionDuration: const Duration(milliseconds: 400),
    pageBuilder: (context, _, _) => _PairingDialog(device: device, code: code),
    transitionBuilder: (context, animation, _, child) {
      final curved = CurvedAnimation(parent: animation, curve: Motion.spring, reverseCurve: Curves.easeIn);
      return FadeTransition(
        opacity: CurvedAnimation(parent: animation, curve: Curves.easeOut),
        child: ScaleTransition(scale: Tween(begin: 0.92, end: 1.0).animate(curved), child: child),
      );
    },
  );
  return result ?? false;
}

class _PairingDialog extends StatefulWidget {
  const _PairingDialog({required this.device, required this.code});

  final Device device;
  final String code;

  @override
  State<_PairingDialog> createState() => _PairingDialogState();
}

class _PairingDialogState extends State<_PairingDialog> with TickerProviderStateMixin {
  late final _intro = AnimationController(vsync: this, duration: const Duration(milliseconds: 1000))..forward();
  late final _countdown = AnimationController(vsync: this, duration: _timeout)
    ..addStatusListener((status) {
      if (status == AnimationStatus.completed && !_paired && mounted) Navigator.of(context).pop(false);
    })
    ..forward();
  bool _paired = false;

  @override
  void dispose() {
    _intro.dispose();
    _countdown.dispose();
    super.dispose();
  }

  Future<void> _accept() async {
    _countdown.stop();
    setState(() => _paired = true);
    await Future<void>.delayed(const Duration(milliseconds: 1200));
    if (mounted) Navigator.of(context).pop(true);
  }

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Material(
          color: c.card,
          shape: continuous(32),
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 460),
            child: Padding(
              padding: const EdgeInsets.all(28),
              child: AnimatedSize(
                duration: Motion.medium,
                curve: Motion.ease,
                child: AnimatedSwitcher(duration: Motion.medium, child: _paired ? _success(c) : _request(c)),
              ),
            ),
          ),
        ),
      ),
    );
  }

  Widget _request(SfColors c) {
    return Column(
      key: const ValueKey('request'),
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          children: [
            Container(
              width: 48,
              height: 48,
              decoration: ShapeDecoration(color: c.accentSoft, shape: continuous(16)),
              child: Icon(Icons.lock_rounded, color: c.accent),
            ),
            const SizedBox(width: 14),
            Expanded(
              child: Text(
                '与“${widget.device.name}”配对',
                style: const TextStyle(fontSize: 22, fontWeight: FontWeight.w700),
              ),
            ),
          ],
        ),
        const SizedBox(height: 12),
        Text('请确认两台电脑上显示的是同一个配对码，一致才确认。', style: TextStyle(color: c.muted, height: 1.6)),
        const SizedBox(height: 26),
        Center(child: _digits(c)),
        const SizedBox(height: 16),
        Center(
          child: Text.rich(
            TextSpan(
              text: '对方指纹 ',
              children: [
                TextSpan(
                  text: widget.device.fingerprint,
                  style: const TextStyle(fontFamily: monoFont),
                ),
              ],
            ),
            style: TextStyle(fontSize: 13, color: c.muted),
          ),
        ),
        const SizedBox(height: 22),
        AnimatedBuilder(
          animation: _countdown,
          builder: (context, _) {
            final left = (_timeout.inSeconds * (1 - _countdown.value)).ceil();
            return Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                ClipRSuperellipse(
                  borderRadius: BorderRadius.circular(3),
                  child: SizedBox(
                    height: 6,
                    child: Stack(
                      children: [
                        Positioned.fill(child: ColoredBox(color: c.card2)),
                        FractionallySizedBox(
                          widthFactor: 1 - _countdown.value,
                          child: ColoredBox(color: c.accent),
                        ),
                      ],
                    ),
                  ),
                ),
                const SizedBox(height: 8),
                Text(
                  '$left 秒后自动拒绝',
                  style: TextStyle(fontSize: 13, color: c.muted, fontFeatures: tabular),
                ),
              ],
            );
          },
        ),
        const SizedBox(height: 22),
        Wrap(
          alignment: WrapAlignment.end,
          spacing: 10,
          runSpacing: 10,
          children: [
            PillButton(label: '拒绝', soft: true, onPressed: () => Navigator.of(context).pop(false)),
            PillButton(label: '数字一致，配对', icon: Icons.check_rounded, primary: true, onPressed: _accept),
          ],
        ),
      ],
    );
  }

  /// 6 个数字格，分两组；入场时从左到右依次弹起。
  Widget _digits(SfColors c) {
    final cells = <Widget>[];
    for (var i = 0; i < widget.code.length; i++) {
      final start = 0.12 + i * 0.07;
      final appear = CurvedAnimation(
        parent: _intro,
        curve: Interval(start, start + 0.5, curve: Motion.spring),
      );
      if (i == 3) cells.add(const SizedBox(width: 10));
      cells.add(
        FadeTransition(
          opacity: CurvedAnimation(parent: _intro, curve: Interval(start, start + 0.3)),
          child: SlideTransition(
            position: Tween(begin: const Offset(0, 0.35), end: Offset.zero).animate(appear),
            child: ScaleTransition(
              scale: Tween(begin: 0.8, end: 1.0).animate(appear),
              child: Container(
                width: 50,
                height: 64,
                margin: const EdgeInsets.symmetric(horizontal: 4),
                alignment: Alignment.center,
                decoration: ShapeDecoration(color: c.card2, shape: continuous(16)),
                child: Text(
                  widget.code[i],
                  style: TextStyle(fontFamily: monoFont, fontSize: 30, fontWeight: FontWeight.w700, color: c.fg),
                ),
              ),
            ),
          ),
        ),
      );
    }
    return Semantics(
      label: '配对码 ${widget.code.substring(0, 3)} ${widget.code.substring(3)}',
      child: ExcludeSemantics(
        child: Row(mainAxisSize: MainAxisSize.min, children: cells),
      ),
    );
  }

  Widget _success(SfColors c) {
    return Padding(
      key: const ValueKey('success'),
      padding: const EdgeInsets.symmetric(vertical: 22),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          TweenAnimationBuilder<double>(
            tween: Tween(begin: 0.3, end: 1),
            duration: const Duration(milliseconds: 600),
            curve: Motion.spring,
            builder: (context, t, child) => Transform.scale(scale: t, child: child),
            child: Container(
              width: 84,
              height: 84,
              decoration: ShapeDecoration(color: c.goodBg, shape: continuous(28)),
              child: Icon(Icons.check_rounded, size: 46, color: c.goodInk),
            ),
          ),
          const SizedBox(height: 14),
          Text('已与“${widget.device.name}”配对', style: const TextStyle(fontSize: 20, fontWeight: FontWeight.w700)),
          const SizedBox(height: 4),
          Text('以后连接不再询问', style: TextStyle(color: c.muted)),
        ],
      ),
    );
  }
}
