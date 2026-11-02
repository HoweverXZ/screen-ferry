// 日志页的“运行日志”分段（ADR 0012）：上面导出诊断文件、复制摘要、去提问题，
// 中间是“详细记录”开关，下面是实时的运行日志。

import 'package:flutter/material.dart';

import '../l10n/l10n.dart';
import '../state/app_scope.dart';
import '../state/app_state.dart';
import '../theme/tokens.dart';
import '../widgets/controls.dart';
import '../widgets/group.dart';
import '../widgets/surface.dart';

/// 运行日志一节的几块卡片，放进页面的 children。
List<Widget> runLogSections() => const [_ExportCard(), _VerboseRow(), _RunLogCard()];

/// 提问题时附上诊断文件。
class _ExportCard extends StatelessWidget {
  const _ExportCard();

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = SfColors.of(context);
    final exported = state.lastExport;
    final online = state.online;
    return SfCard(
      padding: const EdgeInsets.fromLTRB(22, 20, 22, 20),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          BannerRow(
            breakpoint: 640,
            leading: const IconTile(Icons.description_outlined, size: 52, radius: 16, emphasized: true),
            body: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(context.l10n.diagTitle, style: TextStyle(fontSize: 17, fontWeight: FontWeight.w700)),
                const SizedBox(height: 4),
                Text(context.l10n.diagBody, style: TextStyle(fontSize: 14, height: 1.55, color: c.muted)),
              ],
            ),
            trailing: Column(
              crossAxisAlignment: CrossAxisAlignment.end,
              mainAxisSize: MainAxisSize.min,
              children: [
                Wrap(
                  spacing: 8,
                  runSpacing: 8,
                  children: [
                    PillButton(
                      label: context.l10n.diagCopySummary,
                      icon: Icons.copy_rounded,
                      soft: true,
                      onPressed: online ? state.copyDiagnostics : null,
                    ),
                    PillButton(
                      label: context.l10n.diagExport,
                      icon: Icons.download_rounded,
                      primary: true,
                      onPressed: online ? state.exportDiagnostics : null,
                    ),
                  ],
                ),
                const SizedBox(height: 8),
                Wrap(
                  spacing: 16,
                  children: [
                    TextLink(label: context.l10n.diagOpenFolder, onPressed: online ? state.openLogsFolder : null),
                    TextLink(label: context.l10n.diagReport, chevron: true, onPressed: state.openIssue),
                  ],
                ),
              ],
            ),
          ),
          if (exported != null) ...[
            const SizedBox(height: 14),
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
              decoration: ShapeDecoration(color: c.card2, shape: continuous(14)),
              child: Row(
                children: [
                  Icon(Icons.check_rounded, size: 18, color: c.goodInk),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Text.rich(
                      TextSpan(
                        text: context.l10n.diagSaved,
                        children: [
                          TextSpan(
                            text: exported.name,
                            style: const TextStyle(
                              fontFamily: monoFont,
                              fontFamilyFallback: monoFallback,
                              fontSize: 13,
                            ),
                          ),
                          TextSpan(text: ' · ${sizeText(exported.bytes)}'),
                        ],
                      ),
                      style: const TextStyle(fontSize: 13.5),
                    ),
                  ),
                  const SizedBox(width: 10),
                  TextLink(
                    label: state.localIsMac ? context.l10n.diagShowInFinder : context.l10n.diagShowInExplorer,
                    onPressed: online ? state.revealDiagnostics : null,
                  ),
                ],
              ),
            ),
          ],
        ],
      ),
    );
  }
}

/// 详细记录：排查问题时临时打开。
class _VerboseRow extends StatelessWidget {
  const _VerboseRow();

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final since = state.verboseSince;
    final String subtitle;
    if (since == null) {
      subtitle = context.l10n.diagVerboseOff;
    } else {
      final minutes = DateTime.now().difference(since).inMinutes;
      subtitle = minutes < 1 ? context.l10n.diagVerboseJustOn : context.l10n.diagVerboseOnFor(minutes);
    }
    return SfGroup(
      children: [
        SfRow(
          icon: Icons.manage_search_rounded,
          title: context.l10n.diagVerbose,
          subtitle: subtitle,
          trailing: SfSwitch(
            value: since != null,
            label: context.l10n.diagVerbose,
            onChanged: state.online ? state.setVerbose : null,
          ),
        ),
      ],
    );
  }
}

/// 实时的运行日志：旧的在上，新的在下，停在最底下。
class _RunLogCard extends StatefulWidget {
  const _RunLogCard();

  @override
  State<_RunLogCard> createState() => _RunLogCardState();
}

class _RunLogCardState extends State<_RunLogCard> {
  bool _problemsOnly = false;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = SfColors.of(context);
    final lines = _problemsOnly
        ? state.runLog.where((l) => l.level == RunLogLevel.warn || l.level == RunLogLevel.error).toList()
        : state.runLog;
    return SfCard(
      padding: const EdgeInsets.fromLTRB(22, 16, 22, 16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Wrap(
            spacing: 12,
            runSpacing: 10,
            alignment: WrapAlignment.spaceBetween,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text(context.l10n.diagRecent, style: TextStyle(fontSize: 15, fontWeight: FontWeight.w700)),
                  const SizedBox(width: 10),
                  StatusDot(on: state.online),
                  const SizedBox(width: 6),
                  Text(
                    state.online ? context.l10n.diagLive : context.l10n.diagNotConnected,
                    style: TextStyle(fontSize: 12, color: c.muted),
                  ),
                ],
              ),
              SfSegmented<bool>(
                label: context.l10n.diagLevel,
                segments: [(false, context.l10n.logAll), (true, context.l10n.diagProblemsOnly)],
                value: _problemsOnly,
                onChanged: (v) => setState(() => _problemsOnly = v),
              ),
            ],
          ),
          const SizedBox(height: 12),
          SizedBox(
            height: 420,
            child: lines.isEmpty
                ? Center(
                    child: Text(
                      _problemsOnly ? context.l10n.diagNoProblems : context.l10n.diagNoLog,
                      style: TextStyle(color: c.muted),
                    ),
                  )
                : SelectionArea(
                    child: ListView.builder(
                      // 倒过来排：新的一行出现在最底下，停在底部时一直跟着看最新的。
                      reverse: true,
                      itemCount: lines.length,
                      itemBuilder: (context, i) => _RunLogRow(lines[lines.length - 1 - i]),
                    ),
                  ),
          ),
        ],
      ),
    );
  }
}

class _RunLogRow extends StatelessWidget {
  const _RunLogRow(this.line);

  final RunLogLine line;

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    final dark = Theme.of(context).brightness == Brightness.dark;
    final warn = dark ? const Color(0xFFFDBA74) : const Color(0xFF9A3412);
    final (label, labelColor, textColor) = switch (line.level) {
      RunLogLevel.debug => (context.l10n.diagLevelDebug, c.muted, c.muted),
      RunLogLevel.info => (context.l10n.diagLevelInfo, c.muted, c.fg),
      RunLogLevel.warn => (context.l10n.diagLevelWarn, warn, warn),
      RunLogLevel.error => (context.l10n.diagLevelError, c.danger, c.danger),
    };
    const mono = TextStyle(fontFamily: monoFont, fontFamilyFallback: monoFallback, fontSize: 12.5, height: 1.65);
    final strong = line.level == RunLogLevel.warn || line.level == RunLogLevel.error;
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SizedBox(
          width: 74,
          child: Text(line.at, style: mono.copyWith(color: c.muted)),
        ),
        SizedBox(
          width: 42,
          child: Text(
            label,
            style: mono.copyWith(color: labelColor, fontWeight: strong ? FontWeight.w700 : null),
          ),
        ),
        Expanded(
          child: Text(line.text, style: mono.copyWith(color: textColor)),
        ),
      ],
    );
  }
}
