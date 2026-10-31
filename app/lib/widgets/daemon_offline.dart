// 还没连上过后台时的占位页：后台没有运行，或者正在连接。

import 'package:flutter/material.dart';

import '../state/app_scope.dart';
import '../state/app_state.dart';
import '../theme/tokens.dart';
import 'surface.dart';

class DaemonOffline extends StatelessWidget {
  const DaemonOffline({super.key});

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = SfColors.of(context);
    final connecting = state.daemonLink == DaemonLink.connecting;
    return Center(
      child: SingleChildScrollView(
        padding: const EdgeInsets.fromLTRB(28, 48, 28, 28),
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 420),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Container(
                width: 96,
                height: 96,
                decoration: ShapeDecoration(color: c.card, shape: continuous(30)),
                child: Center(child: FerryGlyph(size: 52, color: c.muted)),
              ),
              const SizedBox(height: 22),
              Text(
                connecting ? '正在连接后台…' : '后台没有运行',
                textAlign: TextAlign.center,
                style: const TextStyle(fontSize: 24, fontWeight: FontWeight.w800),
              ),
              const SizedBox(height: 8),
              Text(
                '请先启动 ScreenFerry，连上后会自动显示。',
                textAlign: TextAlign.center,
                style: TextStyle(fontSize: 15, color: c.muted, height: 1.6),
              ),
              const SizedBox(height: 22),
              const _Retrying(),
            ],
          ),
        ),
      ),
    );
  }
}

/// 三个圆点依次起伏：表示一直在重试。
class _Retrying extends StatefulWidget {
  const _Retrying();

  @override
  State<_Retrying> createState() => _RetryingState();
}

class _RetryingState extends State<_Retrying> with SingleTickerProviderStateMixin {
  late final _anim = AnimationController(vsync: this, duration: const Duration(milliseconds: 1400))..repeat();

  @override
  void dispose() {
    _anim.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    return Semantics(
      label: '正在重试',
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          for (var i = 0; i < 3; i++)
            AnimatedBuilder(
              animation: _anim,
              builder: (context, _) {
                // 每个点错开一点相位，起伏一次后歇一会儿。
                final t = ((_anim.value - i * 0.14) % 1.0);
                final lift = t < 0.4 ? Curves.easeInOut.transform(1 - (t / 0.2 - 1).abs()) : 0.0;
                return Container(
                  width: 8,
                  height: 8,
                  margin: const EdgeInsets.symmetric(horizontal: 4),
                  transform: Matrix4.translationValues(0, -6 * lift, 0),
                  decoration: BoxDecoration(shape: BoxShape.circle, color: Color.lerp(c.card3, c.accent, lift)),
                );
              },
            ),
        ],
      ),
    );
  }
}
