// 日志页的“运行日志”分段（ADR 0012）：上面导出诊断文件、复制摘要、去提问题，
// 中间是“详细记录”开关，下面是实时的运行日志。

import 'package:flutter/material.dart';

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
                const Text('提问题时附上诊断文件', style: TextStyle(fontSize: 17, fontWeight: FontWeight.w700)),
                const SizedBox(height: 4),
                Text(
                  '包含版本、系统、设置和最近的运行日志。设备名、用户名和文件名会换成代号，不含剪贴板内容和密钥。',
                  style: TextStyle(fontSize: 14, height: 1.55, color: c.muted),
                ),
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
                      label: '复制摘要',
                      icon: Icons.copy_rounded,
                      soft: true,
                      onPressed: online ? state.copyDiagnostics : null,
                    ),
                    PillButton(
                      label: '导出诊断文件',
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
                    TextLink(label: '打开日志文件夹', onPressed: online ? state.openLogsFolder : null),
                    TextLink(label: '去 GitHub 提问题', chevron: true, onPressed: state.openIssue),
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
                        text: '已存到“下载”：',
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
                    label: state.localIsMac ? '在访达中显示' : '在资源管理器中显示',
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
      subtitle = '排查连接问题时打开，会多记一些细节。重启 ScreenFerry 后自动关闭';
    } else {
      final minutes = DateTime.now().difference(since).inMinutes;
      subtitle = '${minutes < 1 ? '刚刚打开' : '已打开 $minutes 分钟'}。问题出现后导出，再把它关掉';
    }
    return SfGroup(
      children: [
        SfRow(
          icon: Icons.manage_search_rounded,
          title: '详细记录',
          subtitle: subtitle,
          trailing: SfSwitch(value: since != null, label: '详细记录', onChanged: state.online ? state.setVerbose : null),
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
                  const Text('最近的运行日志', style: TextStyle(fontSize: 15, fontWeight: FontWeight.w700)),
                  const SizedBox(width: 10),
                  StatusDot(on: state.online),
                  const SizedBox(width: 6),
                  Text(state.online ? '实时' : '未连接', style: TextStyle(fontSize: 12, color: c.muted)),
                ],
              ),
              SfSegmented<bool>(
                label: '日志级别',
                segments: const [(false, '全部'), (true, '警告和错误')],
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
                    child: Text(_problemsOnly ? '最近没有警告和错误' : '还没有运行日志', style: TextStyle(color: c.muted)),
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
      RunLogLevel.debug => ('调试', c.muted, c.muted),
      RunLogLevel.info => ('信息', c.muted, c.fg),
      RunLogLevel.warn => ('警告', warn, warn),
      RunLogLevel.error => ('错误', c.danger, c.danger),
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
