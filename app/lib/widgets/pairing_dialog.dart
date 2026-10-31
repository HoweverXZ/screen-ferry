// 配对弹窗：后台发来配对请求时弹出，6 位配对码依次弹起；
// 用户确认后等对方也确认，后台说结束了就自动关闭，配对成功时先显示成功动画。

import 'dart:async';

import 'package:flutter/material.dart';

import '../state/app_scope.dart';
import '../state/app_state.dart';
import '../theme/tokens.dart';
import 'surface.dart';

const _timeout = Duration(seconds: 120);

/// 包住页面：有等待核对的配对请求时依次弹出对话框。
class PairingHost extends StatefulWidget {
  const PairingHost({super.key, required this.child});

  final Widget child;

  @override
  State<PairingHost> createState() => _PairingHostState();
}

class _PairingHostState extends State<PairingHost> {
  /// 已经弹过的请求：拒绝后在后台确认结束之前，不要再弹一次。
  final _shown = <int>{};
  bool _showing = false;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _check(AppScope.of(context));
  }

  /// 没有弹窗在显示、又有没弹过的请求时，弹出最早的那个。
  void _check(AppState state) {
    _shown.retainWhere(state.isPromptOpen);
    if (_showing) return;
    final next = state.prompts.where((p) => !_shown.contains(p.id)).firstOrNull;
    if (next == null) return;
    _shown.add(next.id);
    _showing = true;
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      if (!mounted) return;
      await _show(context, next);
      _showing = false;
      // 关闭期间可能又来了新的请求。
      if (mounted) _check(AppScope.read(context));
    });
  }

  @override
  Widget build(BuildContext context) => widget.child;
}

Future<void> _show(BuildContext context, PairingPrompt prompt) {
  return showGeneralDialog<void>(
    context: context,
    barrierDismissible: false,
    barrierLabel: '配对',
    barrierColor: Colors.black.withValues(alpha: 0.34),
    transitionDuration: const Duration(milliseconds: 400),
    pageBuilder: (context, _, _) => _PairingDialog(prompt: prompt),
    transitionBuilder: (context, animation, _, child) {
      final curved = CurvedAnimation(parent: animation, curve: Motion.spring, reverseCurve: Curves.easeIn);
      return FadeTransition(
        opacity: CurvedAnimation(parent: animation, curve: Curves.easeOut),
        child: ScaleTransition(scale: Tween(begin: 0.92, end: 1.0).animate(curved), child: child),
      );
    },
  );
}

enum _Stage { asking, waiting, paired }

class _PairingDialog extends StatefulWidget {
  const _PairingDialog({required this.prompt});

  final PairingPrompt prompt;

  @override
  State<_PairingDialog> createState() => _PairingDialogState();
}

class _PairingDialogState extends State<_PairingDialog> with TickerProviderStateMixin {
  late final _intro = AnimationController(vsync: this, duration: const Duration(milliseconds: 1000))..forward();
  late final _countdown = AnimationController(vsync: this, duration: _timeout)..forward();
  _Stage _stage = _Stage.asking;
  bool _closing = false;
  Timer? _timer;

  PairingPrompt get _p => widget.prompt;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final state = AppScope.of(context);
    final paired = state.peers.any((p) => p.fingerprint == _p.fingerprint);
    if (_stage == _Stage.waiting && paired) {
      // 两边都确认了：先给一个成功动画再关。随后的 build 会显示它。
      _stage = _Stage.paired;
      _closeAfter(const Duration(milliseconds: 1200));
    } else if (!state.isPromptOpen(_p.id) && _stage != _Stage.paired) {
      // 后台说这次配对结束了（对方拒绝、超时或断开）。刚确认时成功的状态可能稍晚一点到，等一会儿。
      _closeAfter(_stage == _Stage.waiting ? const Duration(seconds: 1) : Duration.zero);
    }
  }

  void _closeAfter(Duration delay) {
    if (_closing) return;
    _timer?.cancel();
    _timer = Timer(delay, () {
      if (!mounted || _closing) return;
      _closing = true;
      Navigator.of(context).pop();
    });
  }

  void _reply(bool accept) {
    final state = AppScope.read(context);
    state.replyPairing(_p.id, accept: accept);
    if (!accept) return _closeAfter(Duration.zero);
    _countdown.stop();
    setState(() => _stage = _Stage.waiting);
  }

  @override
  void dispose() {
    _timer?.cancel();
    _intro.dispose();
    _countdown.dispose();
    super.dispose();
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
                child: AnimatedSwitcher(
                  duration: Motion.medium,
                  child: _stage == _Stage.paired ? _success(c) : _request(c),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }

  Widget _request(SfColors c) {
    final waiting = _stage == _Stage.waiting;
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
              child: Text('与“${_p.peerName}”配对', style: const TextStyle(fontSize: 22, fontWeight: FontWeight.w700)),
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
                  text: _p.short,
                  style: const TextStyle(fontFamily: monoFont),
                ),
              ],
            ),
            style: TextStyle(fontSize: 13, color: c.muted),
          ),
        ),
        const SizedBox(height: 22),
        if (waiting)
          Row(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              SizedBox.square(dimension: 16, child: CircularProgressIndicator(strokeWidth: 2.2, color: c.accent)),
              const SizedBox(width: 10),
              Text('已确认，等待对方确认…', style: TextStyle(color: c.muted)),
            ],
          )
        else ...[
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
                    left > 0 ? '$left 秒后自动拒绝' : '已超时',
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
              PillButton(label: '拒绝', soft: true, onPressed: () => _reply(false)),
              PillButton(label: '数字一致，配对', icon: Icons.check_rounded, primary: true, onPressed: () => _reply(true)),
            ],
          ),
        ],
      ],
    );
  }

  /// 6 个数字格，分两组；入场时从左到右依次弹起。
  Widget _digits(SfColors c) {
    final code = _p.digits;
    final cells = <Widget>[];
    for (var i = 0; i < code.length; i++) {
      final start = 0.12 + i * 0.07;
      final appear = CurvedAnimation(
        parent: _intro,
        curve: Interval(start, (start + 0.5).clamp(0, 1), curve: Motion.spring),
      );
      if (i == 3) cells.add(const SizedBox(width: 10));
      cells.add(
        FadeTransition(
          opacity: CurvedAnimation(parent: _intro, curve: Interval(start, (start + 0.3).clamp(0, 1))),
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
                  code[i],
                  style: TextStyle(fontFamily: monoFont, fontSize: 30, fontWeight: FontWeight.w700, color: c.fg),
                ),
              ),
            ),
          ),
        ),
      );
    }
    return Semantics(
      label: '配对码 ${_p.code}',
      child: ExcludeSemantics(
        child: FittedBox(
          fit: BoxFit.scaleDown,
          child: Row(mainAxisSize: MainAxisSize.min, children: cells),
        ),
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
          Text('已与“${_p.peerName}”配对', style: const TextStyle(fontSize: 20, fontWeight: FontWeight.w700)),
          const SizedBox(height: 4),
          Text('以后连接不再询问', style: TextStyle(color: c.muted)),
        ],
      ),
    );
  }
}
