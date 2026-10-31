// 开关与分段控件。

import 'package:flutter/material.dart';

import '../theme/tokens.dart';
import 'pressable.dart';

/// iOS 风格开关：关为浅灰轨道配深灰圆钮，开为强调色轨道配白色圆钮，滑动带回弹。
class SfSwitch extends StatelessWidget {
  const SfSwitch({super.key, required this.value, required this.onChanged, this.label, this.small = false});

  final bool value;
  final ValueChanged<bool>? onChanged;
  final String? label;
  final bool small;

  @override
  Widget build(BuildContext context) {
    final knob = SwitchKnob(on: value, small: small);
    if (onChanged == null) return ExcludeSemantics(child: knob);
    return Semantics(
      toggled: value,
      label: label,
      child: Pressable(onTap: () => onChanged!(!value), radius: 999, pressScale: 0.94, child: knob),
    );
  }
}

/// 开关的外观本身，也用在快捷磁贴里作状态指示。
class SwitchKnob extends StatelessWidget {
  const SwitchKnob({super.key, required this.on, this.small = false});

  final bool on;
  final bool small;

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    final (w, h, k) = small ? (36.0, 22.0, 14.0) : (54.0, 32.0, 22.0);
    return AnimatedContainer(
      duration: Motion.medium,
      curve: Motion.ease,
      width: w,
      height: h,
      padding: EdgeInsets.all((h - k) / 2),
      decoration: ShapeDecoration(color: on ? c.accent : c.track, shape: const StadiumBorder()),
      child: AnimatedAlign(
        duration: Motion.medium,
        curve: Motion.spring,
        alignment: on ? Alignment.centerRight : Alignment.centerLeft,
        child: AnimatedContainer(
          duration: Motion.medium,
          width: k,
          height: k,
          decoration: BoxDecoration(shape: BoxShape.circle, color: on ? Colors.white : c.knobOff),
        ),
      ),
    );
  }
}

/// 分段控件：灰色底槽里一块强调色滑块，选中项变化时弹性滑过去。各段等宽；
/// 地方不够时整体缩窄，文字随之缩小，不会撑出边界。
class SfSegmented<T> extends StatelessWidget {
  const SfSegmented({
    super.key,
    required this.segments,
    required this.value,
    required this.onChanged,
    this.onCard = false,
    this.label,
  });

  final List<(T, String)> segments;
  final T value;

  /// 为 null 时禁用：变淡、不能点。
  final ValueChanged<T>? onChanged;

  /// 放在页面底色上时，底槽用卡片色。
  final bool onCard;
  final String? label;

  static const _gap = 6.0;
  static const _pad = 4.0;
  static const _style = TextStyle(fontSize: 14, fontWeight: FontWeight.w600);

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    final scale = MediaQuery.textScalerOf(context);
    var widest = 0.0;
    for (final (_, text) in segments) {
      final painter = TextPainter(
        text: TextSpan(text: text, style: _style),
        textDirection: TextDirection.ltr,
        textScaler: scale,
      )..layout();
      if (painter.width > widest) widest = painter.width;
    }
    final natural = (widest + 28).ceilToDouble();
    final index = segments.indexWhere((s) => s.$1 == value);
    final gaps = _gap * (segments.length - 1);
    return LayoutBuilder(
      builder: (context, box) {
        final fit = (box.maxWidth - _pad * 2 - gaps) / segments.length;
        final segW = box.maxWidth.isFinite && fit < natural ? fit.floorToDouble() : natural;
        return _build(c, segW, index);
      },
    );
  }

  Widget _build(SfColors c, double segW, int index) {
    final change = onChanged;
    return Semantics(
      label: label,
      enabled: change != null,
      child: AnimatedOpacity(
        duration: Motion.fast,
        opacity: change == null ? 0.5 : 1,
        child: Container(
          padding: const EdgeInsets.all(_pad),
          decoration: ShapeDecoration(color: onCard ? c.card : c.card2, shape: const StadiumBorder()),
          child: SizedBox(
            height: 36,
            width: segW * segments.length + _gap * (segments.length - 1),
            child: Stack(
              children: [
                AnimatedPositioned(
                  duration: Motion.slow,
                  curve: Motion.spring,
                  left: index * (segW + _gap),
                  top: 0,
                  bottom: 0,
                  width: segW,
                  child: DecoratedBox(
                    decoration: ShapeDecoration(color: c.accent, shape: const StadiumBorder()),
                  ),
                ),
                Row(
                  children: [
                    for (var i = 0; i < segments.length; i++) ...[
                      if (i > 0) const SizedBox(width: _gap),
                      SizedBox(
                        width: segW,
                        child: Pressable(
                          onTap: change == null ? null : () => change(segments[i].$1),
                          radius: 999,
                          selected: i == index,
                          semanticLabel: segments[i].$2,
                          child: Center(
                            child: AnimatedDefaultTextStyle(
                              duration: Motion.medium,
                              style: _style.copyWith(color: i == index ? c.onAccent : c.fg),
                              child: Padding(
                                padding: const EdgeInsets.symmetric(horizontal: 6),
                                child: FittedBox(fit: BoxFit.scaleDown, child: Text(segments[i].$2, maxLines: 1)),
                              ),
                            ),
                          ),
                        ),
                      ),
                    ],
                  ],
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
