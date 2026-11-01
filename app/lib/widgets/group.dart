import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';

import '../theme/tokens.dart';
import 'pressable.dart';
import 'surface.dart';

/// 分组列表：一张卡片里若干行，行间有细分隔线。
/// 内容按卡片的圆角裁剪：第一行、最后一行悬停时的底色不会从圆角溢出去。
class SfGroup extends StatelessWidget {
  const SfGroup({super.key, required this.children});

  final List<Widget> children;

  /// 与 [SfCard] 的圆角相同。
  static const _radius = 26.0;

  @override
  Widget build(BuildContext context) {
    final line = SfColors.of(context).line;
    return ClipPath(
      clipper: ShapeBorderClipper(shape: continuous(_radius)),
      child: SfCard(
        padding: const EdgeInsets.symmetric(vertical: 6),
        child: Column(
          children: [
            for (var i = 0; i < children.length; i++) ...[
              if (i > 0) Divider(height: 1, thickness: 1, indent: 20, endIndent: 20, color: line),
              children[i],
            ],
          ],
        ),
      ),
    );
  }
}

/// 列表中的一行：可选的行首图标，标题与说明，行尾控件；可点击。
/// 行尾控件放得下就和文字并排；窗口太窄放不下时，换到文字下面并与标题左对齐。
class SfRow extends StatelessWidget {
  const SfRow({super.key, this.icon, required this.title, this.subtitle, this.trailing, this.onTap, this.mono = false});

  final IconData? icon;
  final String title;
  final String? subtitle;
  final Widget? trailing;
  final VoidCallback? onTap;
  final bool mono;

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    const tile = 40.0;
    const tileGap = 14.0;
    final indent = icon == null ? 0.0 : tile + tileGap;
    final lead = Row(
      children: [
        if (icon != null) ...[IconTile(icon!, size: tile), const SizedBox(width: tileGap)],
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(title, style: const TextStyle(fontSize: 16, fontWeight: FontWeight.w600)),
              if (subtitle != null)
                Padding(
                  padding: const EdgeInsets.only(top: 2),
                  child: Text(
                    subtitle!,
                    style: TextStyle(
                      fontSize: 14,
                      color: c.muted,
                      fontFamily: mono ? monoFont : null,
                      fontFamilyFallback: mono ? monoFallback : null,
                    ),
                  ),
                ),
            ],
          ),
        ),
      ],
    );
    final content = Padding(
      padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 12),
      child: ConstrainedBox(
        constraints: const BoxConstraints(minHeight: 40),
        child: trailing == null ? lead : _LeadTrail(indent: indent, lead: lead, trail: trailing!),
      ),
    );
    if (onTap == null) return content;
    return Pressable(
      onTap: onTap,
      hoverColor: Color.lerp(c.card, c.card2, 0.5),
      radius: 0,
      pressScale: 1,
      semanticLabel: title,
      child: content,
    );
  }
}

/// 两个子组件：文字 [lead] 和控件 [trail]。
/// 并排时文字至少留 [_minLead] 宽、两者垂直居中；放不下时控件换到下一行，左边缩进 [indent]（与标题对齐）。
class _LeadTrail extends MultiChildRenderObjectWidget {
  _LeadTrail({required this.indent, required Widget lead, required Widget trail}) : super(children: [lead, trail]);

  final double indent;

  @override
  RenderObject createRenderObject(BuildContext context) => _RenderLeadTrail(indent);

  @override
  void updateRenderObject(BuildContext context, _RenderLeadTrail renderObject) => renderObject.indent = indent;
}

class _LeadTrailData extends ContainerBoxParentData<RenderBox> {}

class _RenderLeadTrail extends RenderBox
    with
        ContainerRenderObjectMixin<RenderBox, _LeadTrailData>,
        RenderBoxContainerDefaultsMixin<RenderBox, _LeadTrailData> {
  _RenderLeadTrail(this._indent);

  static const _gap = 14.0;
  static const _runGap = 12.0;
  static const _minLead = 160.0;

  double _indent;
  set indent(double value) {
    if (value == _indent) return;
    _indent = value;
    markNeedsLayout();
  }

  @override
  void setupParentData(RenderBox child) {
    if (child.parentData is! _LeadTrailData) child.parentData = _LeadTrailData();
  }

  @override
  void performLayout() {
    final lead = firstChild!;
    final trail = childAfter(lead)!;
    final width = constraints.maxWidth;
    assert(width.isFinite, 'SfRow 需要有限的宽度');
    final leadPos = lead.parentData! as _LeadTrailData;
    final trailPos = trail.parentData! as _LeadTrailData;

    trail.layout(BoxConstraints(maxWidth: width), parentUsesSize: true);
    if (trail.size.width + _gap + _minLead <= width) {
      lead.layout(BoxConstraints.tightFor(width: width - trail.size.width - _gap), parentUsesSize: true);
      final height = math.max(math.max(lead.size.height, trail.size.height), constraints.minHeight);
      leadPos.offset = Offset(0, (height - lead.size.height) / 2);
      trailPos.offset = Offset(width - trail.size.width, (height - trail.size.height) / 2);
      size = constraints.constrain(Size(width, height));
      return;
    }
    lead.layout(BoxConstraints.tightFor(width: width), parentUsesSize: true);
    trail.layout(BoxConstraints(maxWidth: width - _indent), parentUsesSize: true);
    leadPos.offset = Offset.zero;
    trailPos.offset = Offset(_indent, lead.size.height + _runGap);
    size = constraints.constrain(Size(width, lead.size.height + _runGap + trail.size.height));
  }

  @override
  void paint(PaintingContext context, Offset offset) => defaultPaint(context, offset);

  @override
  bool hitTestChildren(BoxHitTestResult result, {required Offset position}) =>
      defaultHitTestChildren(result, position: position);
}
