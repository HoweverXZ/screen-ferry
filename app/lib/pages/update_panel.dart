// 关于页里的版本更新（ADR 0014）：按后台报告的状态显示一张卡片。
// 发现新版本、下载好时是带更新说明的大卡片（说明长了可以滚动）；其他状态是一行说明加按钮。

import 'package:flutter/material.dart';
import 'package:intl/intl.dart';

import '../l10n/l10n.dart';
import '../state/app_scope.dart';
import '../state/app_state.dart';
import '../theme/tokens.dart';
import '../widgets/surface.dart';

class UpdatePanel extends StatelessWidget {
  const UpdatePanel({super.key});

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final l = context.l10n;
    final u = state.updateInfo;
    final latest = u.latest;
    final online = state.online;

    VoidCallback? when(bool ok, VoidCallback action) => online && ok ? action : null;

    final checkButton = PillButton(
      label: l.aboutCheckUpdates,
      icon: Icons.refresh_rounded,
      onPressed: when(true, state.checkUpdate),
    );

    if (latest != null && u.status == UpdateStatus.available && !u.canInstall) {
      return _Row(
        tone: _Tone.plain,
        icon: Icons.lock_outline_rounded,
        title: l.updateManual(latest.version),
        text: l.updateManualHint,
        trailing: PillButton(label: l.updateOpenDownloads, onPressed: state.openReleases),
      );
    }

    switch (u.status) {
      case UpdateStatus.available || UpdateStatus.ready when latest != null:
        return _ReleaseCard(info: u, release: latest, online: online);
      case UpdateStatus.checking:
        return _Row(tone: _Tone.accent, busy: true, title: l.updateChecking, text: l.updateCheckingHint);
      case UpdateStatus.downloading:
        final total = u.total > 0 ? u.total : 1;
        return _Row(
          tone: _Tone.accent,
          icon: Icons.download_rounded,
          title: l.updateDownloading(latest?.version ?? ''),
          text: '${sizeText(u.done)} / ${sizeText(u.total)}',
          progress: (u.done / total).clamp(0, 1).toDouble(),
          trailing: PillButton(label: l.commonCancel, onPressed: when(true, state.cancelUpdate)),
        );
      case UpdateStatus.installing:
        return _Row(
          tone: _Tone.accent,
          busy: true,
          title: l.updateInstalling(latest?.version ?? ''),
          text: l.updateInstallingHint,
        );
      case UpdateStatus.failed:
        final retry = PillButton(label: l.updateRetry, onPressed: when(true, state.checkUpdate));
        final downloads = PillButton(label: l.updateOpenDownloads, onPressed: state.openReleases);
        return switch (u.error) {
          UpdateError.verifyFailed => _Row(
            tone: _Tone.danger,
            icon: Icons.gpp_bad_outlined,
            title: l.updateVerifyFailed,
            text: l.updateVerifyFailedHint,
            trailing: retry,
          ),
          UpdateError.stale => _Row(
            tone: _Tone.warn,
            icon: Icons.schedule_rounded,
            title: l.updateStale,
            text: l.updateStaleHint,
            trailing: downloads,
          ),
          UpdateError.noKeys => _Row(
            tone: _Tone.plain,
            icon: Icons.lock_outline_rounded,
            title: l.updateNoKeys,
            text: l.updateNoKeysHint,
            trailing: downloads,
          ),
          UpdateError.installFailed => _Row(
            tone: _Tone.danger,
            icon: Icons.error_outline_rounded,
            title: l.updateInstallFailed,
            text: u.message,
            trailing: retry,
          ),
          _ => _Row(
            tone: _Tone.danger,
            icon: Icons.cloud_off_rounded,
            title: l.updateUnreachable,
            text: l.updateUnreachableHint,
            trailing: Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                PillButton(label: l.updateBrowserDownload, onPressed: state.openReleases),
                PillButton(label: l.updateRetry, primary: true, onPressed: when(true, state.checkUpdate)),
              ],
            ),
          ),
        };
      case UpdateStatus.upToDate:
        final at = u.checkedAt;
        return _Row(
          tone: _Tone.good,
          icon: Icons.check_rounded,
          title: l.updateUpToDate,
          text: at == null ? state.version : l.updateCheckedAgo(state.version, timeAgo(l, at)),
          trailing: checkButton,
        );
      default:
        return _Row(
          tone: _Tone.plain,
          icon: Icons.refresh_rounded,
          title: l.updateCurrent(state.version),
          text: l.updateCheckingHint,
          trailing: checkButton,
        );
    }
  }
}

enum _Tone { plain, accent, good, warn, danger }

/// 一行：图标、标题和说明，右边是按钮。
class _Row extends StatelessWidget {
  const _Row({
    required this.tone,
    this.icon,
    this.busy = false,
    required this.title,
    required this.text,
    this.progress,
    this.trailing,
  });

  final _Tone tone;
  final IconData? icon;
  final bool busy;
  final String title;
  final String text;
  final double? progress;
  final Widget? trailing;

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    final dark = Theme.of(context).brightness == Brightness.dark;
    final warn = dark ? const Color(0xFFFDBA74) : const Color(0xFF9A3412);
    final (ink, bg) = switch (tone) {
      _Tone.plain => (c.fg, c.card3),
      _Tone.accent => (c.accent, c.accentSoft),
      _Tone.good => (c.goodInk, c.goodBg),
      _Tone.warn => (warn, warn.withValues(alpha: 0.16)),
      _Tone.danger => (c.danger, c.danger.withValues(alpha: 0.14)),
    };
    final tile = Container(
      width: 44,
      height: 44,
      decoration: ShapeDecoration(color: bg, shape: continuous(15)),
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
        if (text.isNotEmpty)
          Padding(
            padding: const EdgeInsets.only(top: 2),
            child: Text(
              text,
              style: TextStyle(fontSize: 13.5, color: c.muted, height: 1.45, fontFeatures: tabular),
            ),
          ),
        if (progress != null)
          Padding(
            padding: const EdgeInsets.only(top: 8),
            child: ClipRRect(
              borderRadius: BorderRadius.circular(4),
              child: LinearProgressIndicator(
                value: progress,
                minHeight: 8,
                color: c.accent,
                backgroundColor: c.card3,
                semanticsLabel: title,
              ),
            ),
          ),
      ],
    );
    return Container(
      padding: const EdgeInsets.fromLTRB(16, 14, 16, 14),
      decoration: ShapeDecoration(color: c.card2, shape: continuous(20)),
      child: trailing == null
          ? Row(
              children: [
                tile,
                const SizedBox(width: 14),
                Expanded(child: body),
              ],
            )
          : BannerRow(gap: 14, leading: tile, body: body, trailing: trailing!),
    );
  }
}

/// 发现新版本、下载好时：版本、日期和大小，可以滚动的更新说明，底部是按钮。
class _ReleaseCard extends StatelessWidget {
  const _ReleaseCard({required this.info, required this.release, required this.online});

  final UpdateInfo info;
  final UpdateRelease release;
  final bool online;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final l = context.l10n;
    final c = SfColors.of(context);
    final ready = info.status == UpdateStatus.ready;
    final zh = Localizations.localeOf(context).languageCode == 'zh';
    final notes = parseNotes(zh ? release.notesZh : release.notesEn);
    final published = release.published;
    final meta = [
      if (published != null) l.updateReleased(DateFormat.MMMd(l.localeName).format(published)),
      if (release.size > 0) sizeText(release.size),
      if (ready) l.updateDownloadedHint,
    ].join(' · ');
    final hint = ready && info.needsLocalConfirm ? l.updateLocalConfirm : l.updateRestartHint;

    return Container(
      padding: const EdgeInsets.fromLTRB(20, 18, 20, 18),
      decoration: ShapeDecoration(color: c.accentSoft, shape: continuous(20)),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Container(
                width: 40,
                height: 40,
                decoration: ShapeDecoration(color: c.accent, shape: continuous(13)),
                child: Icon(Icons.arrow_upward_rounded, size: 21, color: c.onAccent),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      l.updateFound(release.version),
                      style: const TextStyle(fontSize: 17, fontWeight: FontWeight.w700),
                    ),
                    if (meta.isNotEmpty)
                      Text(
                        meta,
                        style: TextStyle(fontSize: 13, color: c.muted, fontFeatures: tabular),
                      ),
                  ],
                ),
              ),
            ],
          ),
          if (notes.isNotEmpty) ...[const SizedBox(height: 14), _Notes(lines: notes)],
          if (release.breaksPeers) ...[
            const SizedBox(height: 12),
            _PeersWarning(text: l.updateBreaksPeersHint(release.version), title: l.updateBreaksPeers),
          ],
          const SizedBox(height: 14),
          Wrap(
            alignment: WrapAlignment.end,
            crossAxisAlignment: WrapCrossAlignment.center,
            spacing: 10,
            runSpacing: 10,
            children: [
              if (ready)
                ConstrainedBox(
                  constraints: const BoxConstraints(maxWidth: 420),
                  child: Text(hint, style: TextStyle(fontSize: 13, color: c.muted, height: 1.4)),
                ),
              if (ready)
                PillButton(label: l.updateLater, onPressed: online ? () => state.dismissUpdate(release.version) : null),
              if (ready)
                PillButton(
                  label: info.needsLocalConfirm ? l.updateInstall : l.updateRestart,
                  icon: Icons.refresh_rounded,
                  primary: true,
                  onPressed: online ? state.installUpdate : null,
                )
              else
                PillButton(
                  label: l.updateDownload,
                  icon: Icons.download_rounded,
                  primary: true,
                  onPressed: online ? state.downloadUpdate : null,
                ),
            ],
          ),
        ],
      ),
    );
  }
}

/// 更新说明：浅色小框，超过几行可以滚动，底部渐隐。
/// 更新说明的一行：分类的小标题（`### 新增`），或者一条（`- …`，旧格式没有前缀）。
typedef NoteLine = ({bool heading, String text});

/// 把清单里的更新说明拆成行。
List<NoteLine> parseNotes(String notes) => [
  for (final raw in notes.split('\n'))
    if (raw.trim() case final line when line.isNotEmpty)
      line.startsWith('###')
          ? (heading: true, text: line.replaceFirst(RegExp(r'^#+\s*'), ''))
          : (heading: false, text: line.replaceFirst(RegExp(r'^[-*•]\s*'), '')),
];

class _Notes extends StatelessWidget {
  const _Notes({required this.lines});

  final List<NoteLine> lines;

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    final long = lines.length > 4;
    final list = Padding(
      padding: EdgeInsets.fromLTRB(14, 12, 14, long ? 18 : 12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          for (final (i, line) in lines.indexed)
            if (line.heading)
              Padding(
                padding: EdgeInsets.only(top: i == 0 ? 0 : 8, bottom: 2),
                child: Text(
                  line.text,
                  style: TextStyle(fontSize: 12.5, fontWeight: FontWeight.w700, color: c.muted),
                ),
              )
            else
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 3),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Padding(
                      padding: const EdgeInsets.only(top: 8, right: 10),
                      child: Container(
                        width: 5,
                        height: 5,
                        decoration: BoxDecoration(color: c.fg, shape: BoxShape.circle),
                      ),
                    ),
                    Expanded(child: Text(line.text, style: const TextStyle(fontSize: 14.5, height: 1.45))),
                  ],
                ),
              ),
        ],
      ),
    );
    final box = ConstrainedBox(
      constraints: const BoxConstraints(maxHeight: 132),
      child: SingleChildScrollView(child: list),
    );
    return DecoratedBox(
      decoration: ShapeDecoration(color: c.card.withValues(alpha: 0.6), shape: continuous(14)),
      child: long
          ? ShaderMask(
              shaderCallback: (rect) => const LinearGradient(
                begin: Alignment.topCenter,
                end: Alignment.bottomCenter,
                colors: [Colors.black, Colors.black, Colors.transparent],
                stops: [0, 0.78, 1],
              ).createShader(rect),
              blendMode: BlendMode.dstIn,
              child: box,
            )
          : box,
    );
  }
}

/// 新版本改了两台之间的通信方式：提醒两台一起更新。
class _PeersWarning extends StatelessWidget {
  const _PeersWarning({required this.title, required this.text});

  final String title;
  final String text;

  @override
  Widget build(BuildContext context) {
    final dark = Theme.of(context).brightness == Brightness.dark;
    final ink = dark ? const Color(0xFFFDBA74) : const Color(0xFF9A3412);
    return Container(
      padding: const EdgeInsets.fromLTRB(14, 10, 14, 10),
      decoration: ShapeDecoration(color: ink.withValues(alpha: 0.12), shape: continuous(14)),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(Icons.sync_alt_rounded, size: 18, color: ink),
          const SizedBox(width: 10),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  title,
                  style: TextStyle(fontSize: 14, fontWeight: FontWeight.w700, color: ink),
                ),
                Text(text, style: TextStyle(fontSize: 13, height: 1.45, color: ink)),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
