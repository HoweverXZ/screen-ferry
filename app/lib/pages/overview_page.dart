// 仪表盘：主卡（连接信息 + 花朵）、延迟、剪贴板、快捷开关、设备、今日。

import 'dart:io';

import 'package:flutter/material.dart';

import '../l10n/l10n.dart';
import '../state/app_scope.dart';
import '../state/app_state.dart';
import '../theme/tokens.dart';
import '../widgets/bars.dart';
import '../widgets/bloom.dart';
import '../widgets/controls.dart';
import '../widgets/page_scaffold.dart';
import '../widgets/pressable.dart';
import '../widgets/status_banners.dart';
import '../widgets/surface.dart';

class OverviewPage extends StatelessWidget {
  const OverviewPage({super.key});

  @override
  Widget build(BuildContext context) {
    return PageScaffold(
      title: context.l10n.overviewTitle,
      children: [
        LayoutBuilder(
          builder: (context, box) {
            final wide = box.maxWidth >= 640;
            const gap = SizedBox(width: 16, height: 16);
            Widget pair(Widget a, Widget b) => wide
                ? IntrinsicHeight(
                    child: Row(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        Expanded(child: a),
                        gap,
                        Expanded(child: b),
                      ],
                    ),
                  )
                : Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      IntrinsicHeight(child: a),
                      gap,
                      IntrinsicHeight(child: b),
                    ],
                  );
            return Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                _HeroCard(wide: wide),
                gap,
                pair(const _LatencyCard(), const _ClipboardCard()),
                gap,
                _QuickTiles(wide: wide),
                gap,
                pair(const _DevicesCard(), const _TodayCard()),
              ],
            );
          },
        ),
      ],
    );
  }
}

// ── 主卡 ─────────────────────────────────────

class _HeroCard extends StatelessWidget {
  const _HeroCard({required this.wide});

  final bool wide;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = SfColors.of(context);
    final l = context.l10n;
    final status = state.status;
    final peer = state.peer;
    final since = state.connectedAt;
    final rtt = state.latestRtt;

    final (title, pill, tone) = switch (status) {
      // 本机给它起过名字就显示起的名字。
      LinkStatus.connected => (state.peerName, l.overviewSharing, PillTone.good),
      LinkStatus.paused => (l.overviewPausedTitle, l.overviewPaused, PillTone.plain),
      LinkStatus.waiting => (l.overviewWaitingTitle, l.overviewWaiting, PillTone.plain),
      LinkStatus.searching => (l.overviewSearchingTitle, l.overviewSearching, PillTone.plain),
      LinkStatus.connecting => (l.overviewConnectingTitle, l.overviewConnecting, PillTone.plain),
      LinkStatus.starting => (l.overviewStartingTitle, l.overviewStarting, PillTone.plain),
      LinkStatus.stopped => (l.overviewStoppedTitle, l.overviewStopped, PillTone.plain),
    };
    final problem = state.problem;
    final hint = switch (status) {
      LinkStatus.connected => null,
      LinkStatus.paused => l.overviewPausedHint,
      LinkStatus.waiting => state.settings.announce ? l.overviewAnnouncingHint : l.overviewNotAnnouncingHint,
      LinkStatus.searching => l.overviewSearchingHint,
      LinkStatus.connecting => l.overviewConnectingHint,
      LinkStatus.starting => l.overviewStartingHint,
      LinkStatus.stopped => problem == null ? l.overviewProblemHint : problemText(l, problem),
    };

    final info = Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          children: [
            const Icon(Icons.monitor_heart_outlined, size: 20),
            const SizedBox(width: 10),
            Text(l.overviewStatus, style: const TextStyle(fontSize: 16, fontWeight: FontWeight.w600)),
            const SizedBox(width: 10),
            Pill(pill, tone: tone),
          ],
        ),
        const SizedBox(height: 18),
        Row(
          children: [
            IconTile(state.peerIcon, size: 66, radius: 22, iconSize: 32, emphasized: state.connected),
            const SizedBox(width: 16),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    title,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(fontSize: wide ? 34 : 28, fontWeight: FontWeight.w800, height: 1.1),
                  ),
                  const SizedBox(height: 10),
                  if (hint != null)
                    Text(hint, style: TextStyle(fontSize: 14, color: c.muted, height: 1.5))
                  else
                    Wrap(
                      spacing: 8,
                      runSpacing: 8,
                      children: [
                        if (rtt != null)
                          Pill(
                            rtt < 1
                                ? l.overviewLatencyGreat
                                : rtt < 5
                                ? l.overviewLatencyGood
                                : l.overviewLatencyHigh,
                            tone: PillTone.good,
                          ),
                        Pill(
                          state.cursorOnPeer || state.cursorPeer != null
                              ? l.overviewCursorOn(state.cursorPeer?.displayName ?? state.peerName)
                              : l.overviewCursorHere,
                          icon: Icons.near_me_rounded,
                        ),
                        if (state.onlineCount > 1)
                          Pill(l.overviewConnectedCount(state.onlineCount), icon: Icons.devices_rounded),
                      ],
                    ),
                ],
              ),
            ),
          ],
        ),
        const SizedBox(height: 20),
        _Facts(
          state.connected && peer != null
              ? [
                  (l.commonConnected, since == null ? '—' : durationText(l, DateTime.now().difference(since))),
                  (l.overviewAddress, peer.addr.isEmpty ? '—' : peer.addr),
                  (l.overviewEncryption, 'TLS 1.3'),
                ]
              : [
                  (l.commonThisComputer, state.localName),
                  (l.overviewFingerprint, state.localFingerprint),
                  (l.overviewEncryption, 'TLS 1.3'),
                ],
        ),
      ],
    );

    final bloom = Bloom(
      active: state.sharing && state.online,
      size: wide ? 236 : 200,
      center: _CoreButton(sharing: state.sharing, onTap: state.online ? () => state.setSharing(!state.sharing) : null),
    );

    return SfCard(
      padding: const EdgeInsets.fromLTRB(28, 24, 24, 24),
      child: wide
          ? Row(
              children: [
                Expanded(child: info),
                const SizedBox(width: 28),
                bloom,
              ],
            )
          : Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Center(child: bloom),
                const SizedBox(height: 12),
                info,
              ],
            ),
    );
  }
}

/// 连接信息：宽时三栏并排，窄（手机大小）时改成上下几行，标签在左、数值在右。
class _Facts extends StatelessWidget {
  const _Facts(this.facts);

  final List<(String, String)> facts;

  static const _valueStyle = TextStyle(fontSize: 17, fontWeight: FontWeight.w700, fontFeatures: tabular);

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    final label = TextStyle(fontSize: 13, color: c.muted);
    return Container(
      decoration: ShapeDecoration(color: c.card2, shape: continuous(18)),
      child: LayoutBuilder(
        builder: (context, box) {
          if (box.maxWidth < 460) {
            return Padding(
              padding: const EdgeInsets.symmetric(horizontal: 18, vertical: 4),
              child: Column(
                children: [
                  for (var i = 0; i < facts.length; i++) ...[
                    if (i > 0) Divider(height: 1, thickness: 1, color: c.line),
                    Padding(
                      padding: const EdgeInsets.symmetric(vertical: 10),
                      child: Row(
                        children: [
                          Text(facts[i].$1, style: label),
                          const SizedBox(width: 12),
                          Expanded(
                            child: Text(
                              facts[i].$2,
                              textAlign: TextAlign.end,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: _valueStyle.copyWith(fontSize: 15),
                            ),
                          ),
                        ],
                      ),
                    ),
                  ],
                ],
              ),
            );
          }
          return Padding(
            padding: const EdgeInsets.symmetric(vertical: 14),
            child: IntrinsicHeight(
              child: Row(
                children: [
                  for (var i = 0; i < facts.length; i++) ...[
                    if (i > 0) VerticalDivider(width: 1, thickness: 1, color: c.line),
                    Expanded(
                      child: Padding(
                        padding: const EdgeInsets.symmetric(horizontal: 18),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(facts[i].$1, style: label),
                            const SizedBox(height: 2),
                            Text(facts[i].$2, maxLines: 1, overflow: TextOverflow.ellipsis, style: _valueStyle),
                          ],
                        ),
                      ),
                    ),
                  ],
                ],
              ),
            ),
          );
        },
      ),
    );
  }
}

/// 花朵中心的圆盘：共享中显示暂停，暂停后显示强调色的播放。
class _CoreButton extends StatelessWidget {
  const _CoreButton({required this.sharing, required this.onTap});

  final bool sharing;

  /// 没连上后台时为 null，按钮不可点。
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    return Pressable(
      onTap: onTap,
      radius: 49,
      pressScale: 0.92,
      tooltip: sharing ? context.l10n.overviewPause : context.l10n.overviewResume,
      semanticLabel: sharing ? context.l10n.overviewPause : context.l10n.overviewResume,
      child: Container(
        width: 98,
        height: 98,
        decoration: BoxDecoration(
          shape: BoxShape.circle,
          color: c.card,
          boxShadow: [BoxShadow(color: c.shadow, blurRadius: 20, offset: const Offset(0, 6))],
        ),
        child: AnimatedSwitcher(
          duration: const Duration(milliseconds: 340),
          switchInCurve: Motion.spring,
          transitionBuilder: (child, animation) => RotationTransition(
            turns: Tween(begin: -0.15, end: 0.0).animate(animation),
            child: ScaleTransition(scale: animation, child: child),
          ),
          child: Icon(
            sharing ? Icons.pause_rounded : Icons.play_arrow_rounded,
            key: ValueKey(sharing),
            size: 40,
            color: sharing ? c.fg : c.accent,
          ),
        ),
      ),
    );
  }
}

// ── 延迟、剪贴板 ─────────────────────────────

class _LatencyCard extends StatelessWidget {
  const _LatencyCard();

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = SfColors.of(context);
    String ms(double? v) => v == null ? '—' : v.toStringAsFixed(2);
    final rtt = state.rtt;
    return SfCard(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          CardHeader(
            icon: Icons.speed_rounded,
            title: context.l10n.overviewRtt,
            trailing: rtt.isEmpty ? null : SideNote(context.l10n.overviewPeak(ms(state.peakRtt))),
          ),
          const SizedBox(height: 10),
          Text.rich(
            TextSpan(
              children: [
                TextSpan(text: ms(state.latestRtt)),
                TextSpan(
                  text: ' ms',
                  style: TextStyle(fontSize: 16, fontWeight: FontWeight.w600, color: c.muted),
                ),
              ],
            ),
            style: const TextStyle(fontSize: 40, fontWeight: FontWeight.w800, height: 1.05, fontFeatures: tabular),
          ),
          const SizedBox(height: 14),
          LatencyBars(values: rtt, active: state.connected),
          const SizedBox(height: 10),
          Text(
            rtt.isEmpty ? context.l10n.overviewRttEmpty : context.l10n.overviewRttAverage(rtt.length, ms(state.avgRtt)),
            style: TextStyle(fontSize: 14, color: c.muted, fontFeatures: tabular),
          ),
        ],
      ),
    );
  }
}

class _ClipboardCard extends StatelessWidget {
  const _ClipboardCard();

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = SfColors.of(context);
    final clip = state.clipboard;
    Widget chip(String text) => Flexible(
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 11, vertical: 5),
        decoration: ShapeDecoration(color: c.card2, shape: const StadiumBorder()),
        child: Text(
          text,
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: const TextStyle(fontSize: 14, fontWeight: FontWeight.w600),
        ),
      ),
    );
    if (clip == null) {
      return SfCard(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            CardHeader(icon: Icons.content_paste_rounded, title: context.l10n.overviewClipboard),
            const SizedBox(height: 14),
            Text(context.l10n.overviewNoClip, style: const TextStyle(fontSize: 15, fontWeight: FontWeight.w600)),
            const SizedBox(height: 4),
            Text(
              state.settings.clipboard ? context.l10n.overviewNoClipHint : context.l10n.overviewClipOffHint,
              style: TextStyle(fontSize: 14, color: c.muted, height: 1.5),
            ),
          ],
        ),
      );
    }
    final here = context.l10n.commonThisComputer;
    final (from, to) = clip.toPeer ? (here, clip.peer) : (clip.peer, here);
    final image = clip.image;
    final meta = switch ((image, clip.chars)) {
      (final i?, _) => context.l10n.overviewClipImage(i.width, i.height, sizeText(clip.bytes)),
      (null, final n?) when clip.rich => context.l10n.overviewClipRich(n),
      (null, final n?) => context.l10n.overviewClipText(n),
      _ => sizeText(clip.bytes),
    };
    final preview = Text(
      clip.preview.isEmpty
          ? (image != null ? context.l10n.overviewAnImage : context.l10n.overviewNotText)
          : clip.preview,
      maxLines: 2,
      overflow: TextOverflow.ellipsis,
      style: TextStyle(fontSize: 15, height: 1.55, color: clip.preview.isEmpty ? c.muted : null),
    );
    return SfCard(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          CardHeader(
            icon: Icons.content_paste_rounded,
            title: context.l10n.overviewClipboard,
            trailing: SideNote(timeAgo(context.l10n, clip.at)),
          ),
          const SizedBox(height: 12),
          Row(
            children: [
              chip(from),
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: 8),
                child: Icon(Icons.arrow_forward_rounded, size: 18, color: c.accent),
              ),
              chip(to),
            ],
          ),
          const SizedBox(height: 12),
          if (image != null)
            Row(
              children: [
                _Thumbnail(image),
                const SizedBox(width: 12),
                Expanded(child: preview),
              ],
            )
          else
            preview,
          const SizedBox(height: 8),
          Text(meta, style: TextStyle(fontSize: 14, color: c.muted)),
        ],
      ),
    );
  }
}

/// 剪贴板图片的缩略图：和两行文字一样高。预览文件不在了就画一个图片图标。
class _Thumbnail extends StatelessWidget {
  const _Thumbnail(this.image);

  final ClipboardImage image;

  static const _size = Size(72, 48);

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    final ratio = MediaQuery.devicePixelRatioOf(context);
    final placeholder = Icon(Icons.image_outlined, size: 22, color: c.muted);
    final path = image.path;
    return Container(
      width: _size.width,
      height: _size.height,
      clipBehavior: Clip.antiAlias,
      decoration: ShapeDecoration(color: c.card2, shape: continuous(12)),
      child: path == null
          ? placeholder
          : Image.file(
              File(path),
              fit: BoxFit.cover,
              // 只按缩略图的大小解码，不把整张截图读进内存。
              cacheWidth: (_size.width * ratio).round(),
              errorBuilder: (_, _, _) => placeholder,
              semanticLabel: context.l10n.overviewClipImageLabel(image.width, image.height),
            ),
    );
  }
}

// ── 快捷开关 ─────────────────────────────────

class _QuickTiles extends StatelessWidget {
  const _QuickTiles({required this.wide});

  final bool wide;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final s = state.settings;
    VoidCallback? toggle(SettingsPatch patch) => state.online ? () => state.updateSettings(patch) : null;
    final tiles = [
      _QuickTile(
        icon: Icons.content_paste_rounded,
        label: context.l10n.overviewClipSync,
        on: s.clipboard,
        onTap: toggle(SettingsPatch(clipboard: !s.clipboard)),
      ),
      _QuickTile(
        icon: Icons.power_settings_new_rounded,
        label: context.l10n.overviewAutostart,
        on: s.autostart,
        onTap: toggle(SettingsPatch(autostart: !s.autostart)),
      ),
      if (state.isServer)
        _QuickTile(
          icon: Icons.wifi_tethering_rounded,
          label: context.l10n.overviewAnnounce,
          on: s.announce,
          onTap: toggle(SettingsPatch(announce: !s.announce)),
        ),
    ];
    if (!wide) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          for (var i = 0; i < tiles.length; i++) ...[if (i > 0) const SizedBox(height: 12), tiles[i]],
        ],
      );
    }
    return Row(
      children: [
        for (var i = 0; i < tiles.length; i++) ...[if (i > 0) const SizedBox(width: 12), Expanded(child: tiles[i])],
      ],
    );
  }
}

class _QuickTile extends StatelessWidget {
  const _QuickTile({required this.icon, required this.label, required this.on, required this.onTap});

  final IconData icon;
  final String label;
  final bool on;

  /// 没连上后台时为 null，磁贴不可点。
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    return Semantics(
      toggled: on,
      child: Pressable(
        onTap: onTap,
        color: c.card,
        hoverColor: Color.lerp(c.card, c.card2, 0.5),
        radius: 22,
        semanticLabel: label,
        padding: const EdgeInsets.fromLTRB(10, 10, 14, 10),
        child: Row(
          children: [
            AnimatedContainer(
              duration: Motion.medium,
              curve: Motion.ease,
              width: 44,
              height: 44,
              decoration: BoxDecoration(shape: BoxShape.circle, color: on ? c.accent : c.card2),
              child: Icon(icon, size: 20, color: on ? c.onAccent : c.fg),
            ),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    label,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(fontSize: 15, fontWeight: FontWeight.w700),
                  ),
                  Text(
                    on ? context.l10n.overviewOn : context.l10n.overviewOff,
                    style: TextStyle(fontSize: 12.5, color: c.muted),
                  ),
                ],
              ),
            ),
            SwitchKnob(on: on, small: true),
          ],
        ),
      ),
    );
  }
}

// ── 设备、今日 ───────────────────────────────

class _DevicesCard extends StatelessWidget {
  const _DevicesCard();

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = SfColors.of(context);
    final peers = state.peers;
    final found = state.found;
    return SfCard(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          CardHeader(
            icon: Icons.devices_rounded,
            title: context.l10n.navDevices,
            trailing: TextLink(
              label: context.l10n.overviewManage,
              chevron: true,
              onPressed: () => state.go(SfPage.devices),
            ),
          ),
          const SizedBox(height: 4),
          if (peers.isEmpty)
            Padding(
              padding: const EdgeInsets.symmetric(vertical: 12),
              child: Text(context.l10n.devicesNonePaired, style: TextStyle(color: c.muted)),
            ),
          for (var i = 0; i < peers.length; i++) ...[
            if (i > 0) Divider(height: 1, thickness: 1, color: c.line),
            Padding(
              padding: const EdgeInsets.symmetric(vertical: 10),
              child: Row(
                children: [
                  IconTile(state.peerIconOf(peers[i]), emphasized: peers[i].online),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          peers[i].displayName,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: const TextStyle(fontWeight: FontWeight.w600),
                        ),
                        Text(seenText(context.l10n, peers[i]), style: TextStyle(fontSize: 13, color: c.muted)),
                      ],
                    ),
                  ),
                  StatusDot(on: peers[i].online),
                ],
              ),
            ),
          ],
          if (found.isNotEmpty) ...[
            const SizedBox(height: 10),
            Pressable(
              onTap: () => state.go(SfPage.devices),
              radius: 999,
              child: DashedBox(
                radius: 999,
                child: SizedBox(
                  height: 42,
                  child: Row(
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: [
                      Icon(Icons.radar_rounded, size: 18, color: c.muted),
                      const SizedBox(width: 8),
                      Text(
                        context.l10n.overviewFoundNew(found.length),
                        style: TextStyle(fontSize: 14, fontWeight: FontWeight.w600, color: c.muted),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ],
          const Spacer(),
          const SizedBox(height: 14),
          Container(
            padding: const EdgeInsets.fromLTRB(14, 10, 8, 10),
            decoration: ShapeDecoration(color: c.card2, shape: continuous(18)),
            child: Row(
              children: [
                IconTile(state.localIcon, emphasized: true),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        context.l10n.overviewThisName(state.localName),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: const TextStyle(fontWeight: FontWeight.w600),
                      ),
                      Text(
                        state.localFingerprint,
                        style: TextStyle(
                          fontSize: 13,
                          color: c.muted,
                          fontFamily: monoFont,
                          fontFamilyFallback: monoFallback,
                        ),
                      ),
                    ],
                  ),
                ),
                SfIconButton(
                  icon: Icons.copy_rounded,
                  tooltip: context.l10n.overviewCopyFingerprint,
                  onPressed: () => copyText(context, state.localFingerprint, context.l10n.devicesFingerprintCopied),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _TodayCard extends StatelessWidget {
  const _TodayCard();

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = SfColors.of(context);
    final today = state.today;
    Widget stat(String value, String label) => Expanded(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            value,
            style: const TextStyle(fontSize: 28, fontWeight: FontWeight.w800, height: 1.1, fontFeatures: tabular),
          ),
          Text(label, style: TextStyle(fontSize: 13, color: c.muted)),
        ],
      ),
    );
    return SfCard(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          CardHeader(
            title: context.l10n.overviewToday,
            trailing: TextLink(label: context.l10n.navLog, chevron: true, onPressed: () => state.go(SfPage.log)),
          ),
          const SizedBox(height: 12),
          Row(
            children: [
              stat('${today.switches}', context.l10n.overviewSwitches(today.switches)),
              stat('${today.clips}', context.l10n.overviewClips(today.clips)),
              stat(today.connectedHours.toStringAsFixed(1), context.l10n.overviewHours),
            ],
          ),
          const Spacer(),
          const SizedBox(height: 20),
          HourBars(counts: today.hours),
        ],
      ),
    );
  }
}
