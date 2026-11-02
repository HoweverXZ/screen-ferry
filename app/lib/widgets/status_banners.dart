// 页面顶部的提示条：后台遇到问题（例如缺少权限）、与后台的连接断开。

import 'package:flutter/material.dart';

import '../l10n/l10n.dart';
import '../state/app_scope.dart';
import '../state/app_state.dart';
import '../theme/tokens.dart';
import 'surface.dart';

/// 当前需要显示的提示条，没有时为空列表。由 PageScaffold 放在每页内容的最上面。
List<Widget> statusBanners(BuildContext context) {
  final state = AppScope.of(context);
  final problem = state.problem;
  return [
    if (state.disconnected)
      _Banner(
        key: const ValueKey('banner-disconnected'),
        icon: Icons.sync_problem_rounded,
        title: context.l10n.bannerDaemonLost,
        busy: true,
      )
    else if (problem != null)
      _Banner(
        key: const ValueKey('banner-problem'),
        icon: problem.kind == ProblemKind.permission ? Icons.back_hand_rounded : Icons.warning_amber_rounded,
        title: switch (problem.kind) {
          ProblemKind.permission => context.l10n.bannerPermission,
          ProblemKind.portInUse => context.l10n.bannerPortInUse,
          ProblemKind.other => context.l10n.bannerOther,
        },
        text: problemText(context.l10n, problem),
        warn: true,
        action: problem.kind == ProblemKind.permission
            ? PillButton(
                label: context.l10n.bannerOpenSystemSettings,
                icon: Icons.open_in_new_rounded,
                onPressed: state.openPermissionSettings,
              )
            : null,
      ),
    // 新版本下载好了：只在仪表盘上提一下。
    if (state.showUpdateBanner && state.page == SfPage.overview && state.openedDevice == null)
      _Banner(
        key: const ValueKey('banner-update'),
        icon: Icons.arrow_upward_rounded,
        title: context.l10n.updateBannerTitle(state.updateInfo.readyVersion!),
        text: context.l10n.updateBannerHint,
        action: Wrap(
          spacing: 8,
          runSpacing: 8,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: [
            PillButton(
              label: context.l10n.updateLater,
              soft: true,
              onPressed: () => state.dismissUpdate(state.updateInfo.readyVersion!),
            ),
            PillButton(
              label: context.l10n.updateRestart,
              primary: true,
              onPressed: state.online ? state.installUpdate : null,
            ),
          ],
        ),
      ),
  ];
}

/// 后台遇到的问题，用当前语言说明。认不出的问题照后台给的原话显示。
String problemText(AppLocalizations l, Problem problem) => switch (problem.kind) {
  ProblemKind.permission => l.problemPermission,
  ProblemKind.portInUse when problem.port != null => l.problemPortInUse(problem.port!),
  _ => problem.message,
};

class _Banner extends StatelessWidget {
  const _Banner({
    super.key,
    required this.icon,
    required this.title,
    this.text,
    this.action,
    this.warn = false,
    this.busy = false,
  });

  final IconData icon;
  final String title;
  final String? text;
  final Widget? action;
  final bool warn;
  final bool busy;

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    final ink = warn ? c.danger : c.accent;
    final tile = Container(
      width: 44,
      height: 44,
      decoration: ShapeDecoration(color: ink.withValues(alpha: 0.12), shape: continuous(15)),
      child: busy
          ? Padding(
              padding: const EdgeInsets.all(13),
              child: CircularProgressIndicator(strokeWidth: 2.4, color: ink),
            )
          : Icon(icon, size: 22, color: ink),
    );
    final body = Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(title, style: const TextStyle(fontSize: 16, fontWeight: FontWeight.w700)),
        if (text != null && text!.isNotEmpty)
          Padding(
            padding: const EdgeInsets.only(top: 2),
            child: Text(text!, style: TextStyle(fontSize: 14, color: c.muted, height: 1.5)),
          ),
      ],
    );
    return Semantics(
      liveRegion: true,
      child: Container(
        padding: const EdgeInsets.fromLTRB(16, 14, 16, 14),
        decoration: ShapeDecoration(
          color: Color.lerp(c.card, ink, warn ? 0.07 : 0.05),
          shape: continuous(22, side: BorderSide(color: ink.withValues(alpha: 0.22))),
        ),
        child: action == null
            ? Row(
                children: [
                  tile,
                  const SizedBox(width: 14),
                  Expanded(child: body),
                ],
              )
            : BannerRow(gap: 14, leading: tile, body: body, trailing: action!),
      ),
    );
  }
}
