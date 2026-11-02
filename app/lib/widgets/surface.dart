// 卡片、标签、图标块、键帽等基础外观。

import 'dart:math';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../state/app_scope.dart';
import '../theme/tokens.dart';
import 'pressable.dart';

/// 白色卡片：大圆角、无边框、无阴影。
class SfCard extends StatelessWidget {
  const SfCard({super.key, required this.child, this.padding = const EdgeInsets.fromLTRB(24, 22, 24, 22), this.color});

  final Widget child;
  final EdgeInsetsGeometry padding;
  final Color? color;

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      decoration: ShapeDecoration(color: color ?? SfColors.of(context).card, shape: continuous(26)),
      child: Padding(padding: padding, child: child),
    );
  }
}

/// 横幅里的一行：左边图标、中间文字、右边按钮。
/// 宽度够时按钮贴右，右侧留白与左侧相同（都由外层卡片的内边距决定）；太窄时按钮换到下一行。
class BannerRow extends StatelessWidget {
  const BannerRow({
    super.key,
    required this.leading,
    required this.body,
    required this.trailing,
    this.gap = 18,
    this.breakpoint = 520,
  });

  final Widget leading;
  final Widget body;
  final Widget trailing;
  final double gap;

  /// 窄于这个宽度时按钮换行。
  final double breakpoint;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, box) {
        final head = [leading, SizedBox(width: gap), Expanded(child: body)];
        if (box.maxWidth < breakpoint) {
          return Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(children: head),
              const SizedBox(height: 14),
              trailing,
            ],
          );
        }
        return Row(
          children: [
            ...head,
            SizedBox(width: gap),
            trailing,
          ],
        );
      },
    );
  }
}

/// 卡片标题行：图标、标题，右侧可放说明文字或按钮。
class CardHeader extends StatelessWidget {
  const CardHeader({super.key, this.icon, required this.title, this.trailing});

  final IconData? icon;
  final String title;
  final Widget? trailing;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: 30,
      child: Row(
        children: [
          if (icon != null) ...[Icon(icon, size: 20), const SizedBox(width: 10)],
          Expanded(
            child: Text(title, style: const TextStyle(fontSize: 16, fontWeight: FontWeight.w600)),
          ),
          ?trailing,
        ],
      ),
    );
  }
}

/// 卡片右上角的灰色说明文字。
class SideNote extends StatelessWidget {
  const SideNote(this.text, {super.key});

  final String text;

  @override
  Widget build(BuildContext context) => Text(
    text,
    style: TextStyle(fontSize: 14, color: SfColors.of(context).muted, fontFeatures: tabular),
  );
}

enum PillTone { plain, good, accent }

/// 胶囊标签。
class Pill extends StatelessWidget {
  const Pill(this.text, {super.key, this.tone = PillTone.plain, this.icon});

  final String text;
  final PillTone tone;
  final IconData? icon;

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    final (bg, fg) = switch (tone) {
      PillTone.plain => (c.card2, c.muted),
      PillTone.good => (c.goodBg, c.goodInk),
      PillTone.accent => (c.accent, c.onAccent),
    };
    return AnimatedContainer(
      duration: Motion.medium,
      height: 28,
      padding: const EdgeInsets.symmetric(horizontal: 11),
      decoration: ShapeDecoration(color: bg, shape: const StadiumBorder()),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          if (icon != null) ...[Icon(icon, size: 15, color: fg), const SizedBox(width: 5)],
          Flexible(
            child: Text(
              text,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(fontSize: 13, fontWeight: FontWeight.w600, color: fg, fontFeatures: tabular),
            ),
          ),
        ],
      ),
    );
  }
}

/// 圆角方块里的图标，用于列表行首、设备头像。
class IconTile extends StatelessWidget {
  const IconTile(this.icon, {super.key, this.size = 40, this.radius = 14, this.emphasized = false, this.iconSize});

  final IconData icon;
  final double size;
  final double radius;
  final bool emphasized;
  final double? iconSize;

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    return AnimatedContainer(
      duration: Motion.medium,
      width: size,
      height: size,
      decoration: ShapeDecoration(color: emphasized ? c.accentSoft : c.card2, shape: continuous(radius)),
      child: Icon(icon, size: iconSize ?? size * 0.5, color: emphasized ? c.accent : c.fg),
    );
  }
}

/// 状态圆点：实心是在线 / 共享中，空心是离线 / 已暂停。
class StatusDot extends StatelessWidget {
  const StatusDot({super.key, required this.on});

  final bool on;

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    return AnimatedContainer(
      duration: Motion.medium,
      width: 9,
      height: 9,
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        color: on ? c.goodInk : c.goodInk.withValues(alpha: 0),
        border: on ? null : Border.all(color: c.muted, width: 2),
      ),
    );
  }
}

/// 分组标题。
class SectionTitle extends StatelessWidget {
  const SectionTitle(this.title, {super.key, this.note});

  final String title;
  final String? note;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(4, 14, 4, 0),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.baseline,
        textBaseline: TextBaseline.alphabetic,
        children: [
          Flexible(
            child: Text(
              title,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: const TextStyle(fontSize: 18, fontWeight: FontWeight.w700),
            ),
          ),
          if (note != null) ...[
            const SizedBox(width: 10),
            Flexible(
              child: Text(
                note!,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(fontSize: 14, color: SfColors.of(context).muted),
              ),
            ),
          ],
        ],
      ),
    );
  }
}

/// 快捷键键帽。
class KeyCap extends StatelessWidget {
  const KeyCap(this.label, {super.key});

  final String label;

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    return Container(
      constraints: const BoxConstraints(minWidth: 34),
      height: 32,
      padding: const EdgeInsets.symmetric(horizontal: 10),
      alignment: Alignment.center,
      decoration: ShapeDecoration(
        color: c.card2,
        shape: continuous(10, side: BorderSide(color: c.line)),
        shadows: [BoxShadow(color: c.line, offset: const Offset(0, 1.5))],
      ),
      child: Text(label, style: const TextStyle(fontSize: 14, fontWeight: FontWeight.w600)),
    );
  }
}

class KeyCombo extends StatelessWidget {
  const KeyCombo(this.keys, {super.key});

  final List<String> keys;

  @override
  Widget build(BuildContext context) => Row(
    mainAxisSize: MainAxisSize.min,
    children: [
      for (var i = 0; i < keys.length; i++) ...[if (i > 0) const SizedBox(width: 6), KeyCap(keys[i])],
    ],
  );
}

/// 胶囊按钮。
class PillButton extends StatelessWidget {
  const PillButton({
    super.key,
    required this.label,
    this.icon,
    this.onPressed,
    this.primary = false,
    this.soft = false,
  });

  final String label;
  final IconData? icon;
  final VoidCallback? onPressed;
  final bool primary;
  final bool soft;

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    final bg = primary ? c.accent : (soft ? c.card2 : c.card);
    final fg = primary ? c.onAccent : c.fg;
    return Pressable(
      onTap: onPressed,
      color: bg,
      hoverColor: primary ? Color.lerp(c.accent, Colors.black, 0.08) : (soft ? c.card3 : c.card2),
      radius: 999,
      pressScale: 0.96,
      padding: const EdgeInsets.symmetric(horizontal: 18),
      child: SizedBox(
        height: 42,
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            if (icon != null) ...[Icon(icon, size: 18, color: fg), const SizedBox(width: 8)],
            Flexible(
              child: Text(
                label,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(fontSize: 15, fontWeight: FontWeight.w600, color: fg),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// 小号胶囊按钮，用在卡片底部。
class SmallButton extends StatelessWidget {
  const SmallButton({super.key, required this.label, this.onPressed, this.tone = PillTone.plain, this.danger = false});

  final String label;
  final VoidCallback? onPressed;
  final PillTone tone;
  final bool danger;

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    final (bg, fg) = danger
        ? (c.danger, c.card)
        : switch (tone) {
            PillTone.accent => (c.accent, c.onAccent),
            _ => (c.card2, c.fg),
          };
    return Pressable(
      onTap: onPressed,
      color: bg,
      hoverColor: Color.lerp(bg, c.fg, 0.06),
      radius: 999,
      padding: const EdgeInsets.symmetric(horizontal: 14),
      child: SizedBox(
        height: 34,
        child: Center(
          widthFactor: 1,
          child: Text(
            label,
            style: TextStyle(fontSize: 14, fontWeight: FontWeight.w600, color: fg),
          ),
        ),
      ),
    );
  }
}

/// 灰色文字按钮，悬停时出现底色。
class TextLink extends StatelessWidget {
  const TextLink({super.key, required this.label, this.onPressed, this.chevron = false, this.danger = false});

  final String label;
  final VoidCallback? onPressed;
  final bool chevron;
  final bool danger;

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    final color = danger ? c.danger : c.muted;
    return Pressable(
      onTap: onPressed,
      hoverColor: c.card2,
      radius: 10,
      padding: EdgeInsets.fromLTRB(10, 4, chevron ? 4 : 10, 4),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Flexible(
            child: Text(
              label,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(fontSize: 14, fontWeight: FontWeight.w600, color: color),
            ),
          ),
          if (chevron) Icon(Icons.chevron_right_rounded, size: 18, color: color),
        ],
      ),
    );
  }
}

/// 只有图标的方形按钮。
class SfIconButton extends StatelessWidget {
  const SfIconButton({super.key, required this.icon, required this.tooltip, this.onPressed, this.size = 36});

  final IconData icon;
  final String tooltip;
  final VoidCallback? onPressed;
  final double size;

  @override
  Widget build(BuildContext context) {
    return Pressable(
      onTap: onPressed,
      hoverColor: SfColors.of(context).card2,
      radius: 12,
      tooltip: tooltip,
      semanticLabel: tooltip,
      child: SizedBox.square(dimension: size, child: Icon(icon, size: 20)),
    );
  }
}

/// 虚线圆角框：表示“还没配对”“还没有内容”。
class DashedBox extends StatelessWidget {
  const DashedBox({super.key, required this.child, this.radius = 26, this.color});

  final Widget child;
  final double radius;
  final Color? color;

  @override
  Widget build(BuildContext context) {
    return CustomPaint(
      painter: _DashPainter(color: color ?? SfColors.of(context).card3, radius: radius),
      child: child,
    );
  }
}

class _DashPainter extends CustomPainter {
  _DashPainter({required this.color, required this.radius});

  final Color color;
  final double radius;

  @override
  void paint(Canvas canvas, Size size) {
    final rect = RRect.fromRectAndRadius((Offset.zero & size).deflate(1), Radius.circular(radius));
    final paint = Paint()
      ..color = color
      ..style = PaintingStyle.stroke
      ..strokeWidth = 2;
    for (final metric in (Path()..addRRect(rect)).computeMetrics()) {
      for (var d = 0.0; d < metric.length; d += 11) {
        canvas.drawPath(metric.extractPath(d, min(d + 6, metric.length)), paint);
      }
    }
  }

  @override
  bool shouldRepaint(_DashPainter old) => old.color != color || old.radius != radius;
}

/// 复制文字并提示。
void copyText(BuildContext context, String text, String done) {
  Clipboard.setData(ClipboardData(text: text));
  AppScope.read(context).showToast(done);
}
