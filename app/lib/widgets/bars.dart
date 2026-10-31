import 'dart:math';

import 'package:flutter/material.dart';

import '../theme/tokens.dart';

/// 往返延迟柱状图：最新的一根最亮，其余淡一些。
class LatencyBars extends StatelessWidget {
  const LatencyBars({super.key, required this.values, required this.active});

  final List<double> values;
  final bool active;

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    final top = max(0.8, values.reduce(max) * 1.15);
    return RepaintBoundary(
      child: SizedBox(
        height: 58,
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.end,
          children: [
            for (var i = 0; i < values.length; i++) ...[
              if (i > 0) const SizedBox(width: 3),
              Expanded(
                child: AnimatedContainer(
                  duration: const Duration(milliseconds: 300),
                  curve: Motion.ease,
                  height: max(6, values[i] / top * 58),
                  decoration: ShapeDecoration(
                    color: (active ? c.accent : c.muted).withValues(alpha: i == values.length - 1 ? 1 : 0.25),
                    shape: continuous(3),
                  ),
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

/// 今日按小时的切换次数，当前小时用强调色。
class HourBars extends StatelessWidget {
  const HourBars({super.key, required this.counts});

  final List<int> counts;

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    final now = DateTime.now().hour;
    final top = max(1, counts.reduce(max));
    return Semantics(
      label: '今日各小时的切换次数',
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          SizedBox(
            height: 64,
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.end,
              children: [
                for (var i = 0; i < counts.length; i++) ...[
                  if (i > 0) const SizedBox(width: 3),
                  Expanded(
                    child: AnimatedContainer(
                      duration: Motion.medium,
                      height: max(4, counts[i] / top * 64),
                      decoration: ShapeDecoration(color: i == now ? c.accent : c.card3, shape: continuous(3)),
                    ),
                  ),
                ],
              ],
            ),
          ),
          const SizedBox(height: 6),
          DefaultTextStyle(
            style: TextStyle(fontSize: 12, color: c.muted),
            child: const Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [Text('0 时'), Text('12 时'), Text('现在')],
            ),
          ),
        ],
      ),
    );
  }
}
