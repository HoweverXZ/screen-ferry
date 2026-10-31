import 'package:flutter/material.dart';

import '../theme/tokens.dart';

/// 可点击的表面：悬停变色、按下轻微缩小（带回弹）、键盘聚焦时显示描边，回车和空格都能触发。
class Pressable extends StatefulWidget {
  const Pressable({
    super.key,
    required this.child,
    this.onTap,
    this.color,
    this.hoverColor,
    this.radius = 16,
    this.padding,
    this.pressScale = 0.97,
    this.semanticLabel,
    this.tooltip,
    this.selected,
  });

  final Widget child;
  final VoidCallback? onTap;
  final Color? color;
  final Color? hoverColor;
  final double radius;
  final EdgeInsetsGeometry? padding;
  final double pressScale;
  final String? semanticLabel;
  final String? tooltip;
  final bool? selected;

  @override
  State<Pressable> createState() => _PressableState();
}

class _PressableState extends State<Pressable> {
  bool _hover = false;
  bool _down = false;
  bool _focus = false;

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    final enabled = widget.onTap != null;
    final shape = continuous(widget.radius);
    final hover = widget.hoverColor;
    // 没有底色时用“悬停色的全透明版”，不用 Colors.transparent：后者是透明的黑色，
    // 两者之间插值会先变暗再变亮，悬停和点击时会闪一下深灰。
    final idle = widget.color ?? hover?.withValues(alpha: 0) ?? const Color(0x00FFFFFF);
    final fill = _hover && enabled && hover != null ? hover : idle;

    Widget body = AnimatedContainer(
      duration: Motion.fast,
      curve: Motion.ease,
      padding: widget.padding,
      decoration: ShapeDecoration(color: fill, shape: shape),
      foregroundDecoration: _focus
          ? ShapeDecoration(
              shape: continuous(widget.radius, side: BorderSide(color: c.accent, width: 2.5)),
            )
          : null,
      child: widget.child,
    );
    body = AnimatedScale(
      scale: _down ? widget.pressScale : 1,
      duration: Motion.fast,
      curve: Motion.spring,
      child: body,
    );
    body = FocusableActionDetector(
      enabled: enabled,
      mouseCursor: enabled ? SystemMouseCursors.click : MouseCursor.defer,
      onShowHoverHighlight: (v) => setState(() => _hover = v),
      onShowFocusHighlight: (v) => setState(() => _focus = v),
      actions: {
        ActivateIntent: CallbackAction<ActivateIntent>(
          onInvoke: (_) {
            widget.onTap?.call();
            return null;
          },
        ),
      },
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTapDown: enabled ? (_) => setState(() => _down = true) : null,
        onTapUp: enabled ? (_) => setState(() => _down = false) : null,
        onTapCancel: enabled ? () => setState(() => _down = false) : null,
        onTap: widget.onTap,
        child: body,
      ),
    );
    body = Semantics(button: enabled, selected: widget.selected, label: widget.semanticLabel, child: body);
    if (widget.tooltip != null) body = Tooltip(message: widget.tooltip!, child: body);
    return body;
  }
}
