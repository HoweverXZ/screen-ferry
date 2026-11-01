// 日志：光标切换、剪贴板同步、设备与连接的记录，可筛选、可清空，新记录实时插到最上面。
// 最后一个分段“运行日志”是后台的运行日志和诊断文件导出（见 run_log_view.dart）。

import 'package:flutter/material.dart';

import '../state/app_scope.dart';
import '../state/app_state.dart';
import '../theme/tokens.dart';
import '../widgets/controls.dart';
import '../widgets/page_scaffold.dart';
import '../widgets/surface.dart';
import 'run_log_view.dart';

enum _Filter { all, cursor, clip, file, link, run }

class LogPage extends StatefulWidget {
  const LogPage({super.key});

  @override
  State<LogPage> createState() => _LogPageState();
}

class _LogPageState extends State<LogPage> {
  _Filter _filter = _Filter.all;

  /// 打开页面时已有的记录不播放进场动画，之后新来的才播放。
  late final int _openedAt = AppScope.read(context).log.firstOrNull?.id ?? -1;

  late final AppState _state = AppScope.read(context);

  void _select(_Filter filter) {
    if (filter == _filter) return;
    if (filter == _Filter.run || _filter == _Filter.run) _state.watchRunLog(filter == _Filter.run);
    setState(() => _filter = filter);
  }

  @override
  void dispose() {
    if (_filter == _Filter.run) _state.watchRunLog(false);
    super.dispose();
  }

  bool _match(LogEntry e) => switch (_filter) {
    _Filter.all => true,
    _Filter.cursor => e.kind == LogKind.cursor,
    _Filter.clip => e.kind == LogKind.clip,
    _Filter.file => e.kind == LogKind.file,
    _Filter.link => e.kind == LogKind.link,
    _Filter.run => false,
  };

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = SfColors.of(context);
    final run = _filter == _Filter.run;
    final items = state.log.where(_match).toList();
    final bytes = state.runLogBytes;
    final count = run
        ? (bytes == null ? '' : '${sizeText(bytes)} · ${_thousands(state.runLogLines ?? 0)} 行')
        : '共 ${items.length} 条';
    return PageScaffold(
      title: '日志',
      actions: [
        if (!run)
          PillButton(label: '清空', icon: Icons.delete_outline_rounded, onPressed: state.online ? state.clearLog : null),
      ],
      children: [
        Wrap(
          spacing: 12,
          runSpacing: 12,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: [
            SfSegmented<_Filter>(
              label: '筛选',
              onCard: true,
              segments: const [
                (_Filter.all, '全部'),
                (_Filter.cursor, '光标'),
                (_Filter.clip, '剪贴板'),
                (_Filter.file, '文件'),
                (_Filter.link, '设备与连接'),
                (_Filter.run, '运行日志'),
              ],
              value: _filter,
              onChanged: _select,
            ),
            Text(
              count,
              style: TextStyle(color: c.muted, fontFeatures: tabular),
            ),
          ],
        ),
        if (run)
          ...runLogSections()
        else
          SfCard(
            padding: const EdgeInsets.symmetric(vertical: 6),
            child: items.isEmpty
                ? Padding(
                    padding: const EdgeInsets.symmetric(vertical: 34),
                    child: Center(
                      child: Text('暂无记录', style: TextStyle(color: c.muted)),
                    ),
                  )
                : Column(
                    children: [
                      for (var i = 0; i < items.length; i++) ...[
                        if (i > 0) Divider(height: 1, thickness: 1, indent: 20, endIndent: 20, color: c.line),
                        _LogRow(key: ValueKey(items[i].id), entry: items[i], fresh: items[i].id > _openedAt),
                      ],
                    ],
                  ),
          ),
      ],
    );
  }
}

class _LogRow extends StatelessWidget {
  const _LogRow({super.key, required this.entry, required this.fresh});

  final LogEntry entry;
  final bool fresh;

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    Widget row = Padding(
      padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 10),
      child: Row(
        children: [
          SizedBox(
            width: 70,
            child: Text(
              clockTime(entry.at),
              style: TextStyle(fontFamily: monoFont, fontFamilyFallback: monoFallback, fontSize: 13, color: c.muted),
            ),
          ),
          Container(
            width: 34,
            height: 34,
            decoration: BoxDecoration(shape: BoxShape.circle, color: c.card2),
            child: Icon(entry.icon.data, size: 17),
          ),
          const SizedBox(width: 14),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(entry.text),
                Text(entry.detail, style: TextStyle(fontSize: 13, color: c.muted)),
              ],
            ),
          ),
          const SizedBox(width: 12),
          Pill(entry.kind.label),
        ],
      ),
    );
    if (fresh) {
      row = TweenAnimationBuilder<double>(
        tween: Tween(begin: 0, end: 1),
        duration: const Duration(milliseconds: 500),
        curve: Motion.ease,
        builder: (context, t, child) => Opacity(
          opacity: t,
          child: Transform.translate(offset: Offset(0, -10 * (1 - t)), child: child),
        ),
        child: row,
      );
    }
    return row;
  }
}

/// 千位分隔：18241 → 18,241。
String _thousands(int n) => n.toString().replaceAllMapped(RegExp(r'\B(?=(\d{3})+(?!\d))'), (_) => ',');
