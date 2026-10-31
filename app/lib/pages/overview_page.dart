// 仪表盘：主卡（连接信息 + 花朵）、延迟、剪贴板、快捷开关、设备、今日。

import 'package:flutter/material.dart';

import '../state/app_scope.dart';
import '../state/app_state.dart';
import '../theme/tokens.dart';
import '../widgets/bars.dart';
import '../widgets/bloom.dart';
import '../widgets/controls.dart';
import '../widgets/page_scaffold.dart';
import '../widgets/pressable.dart';
import '../widgets/surface.dart';

class OverviewPage extends StatelessWidget {
  const OverviewPage({super.key});

  @override
  Widget build(BuildContext context) {
    return PageScaffold(
      title: '仪表盘',
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
    final mins = DateTime.now().difference(state.connectedAt).inMinutes;
    final onPeer = state.sharing && state.cursorOnPeer;

    final info = Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          children: [
            const Icon(Icons.monitor_heart_outlined, size: 20),
            const SizedBox(width: 10),
            const Text('状态', style: TextStyle(fontSize: 16, fontWeight: FontWeight.w600)),
            const SizedBox(width: 10),
            Pill(state.sharing ? '共享中' : '已暂停', tone: state.sharing ? PillTone.good : PillTone.plain),
          ],
        ),
        const SizedBox(height: 18),
        Row(
          children: [
            const IconTile(Icons.desktop_windows_outlined, size: 66, radius: 22, iconSize: 32),
            const SizedBox(width: 16),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    state.peerName,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(fontSize: wide ? 34 : 28, fontWeight: FontWeight.w800, height: 1.1),
                  ),
                  const SizedBox(height: 10),
                  Wrap(
                    spacing: 8,
                    runSpacing: 8,
                    children: [
                      Pill(state.peerOs),
                      Pill(state.latestRtt < 1 ? '延迟 优' : '延迟 一般', tone: PillTone.good),
                      Pill(onPeer ? '光标在 ${state.peerName}' : '光标在本机', icon: Icons.near_me_rounded),
                    ],
                  ),
                ],
              ),
            ),
          ],
        ),
        const SizedBox(height: 20),
        _Facts([('已连接', '${mins ~/ 60} 小时 ${mins % 60} 分'), ('地址', state.peerAddress), ('加密', 'TLS 1.3')]),
      ],
    );

    final bloom = Bloom(
      active: state.sharing,
      size: wide ? 236 : 200,
      center: _CoreButton(sharing: state.sharing, onTap: () => state.setSharing(!state.sharing)),
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
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    return Pressable(
      onTap: onTap,
      radius: 49,
      pressScale: 0.92,
      tooltip: sharing ? '暂停共享' : '继续共享',
      semanticLabel: sharing ? '暂停共享' : '继续共享',
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
    String ms(double v) => v.toStringAsFixed(2);
    return SfCard(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          CardHeader(icon: Icons.speed_rounded, title: '往返延迟', trailing: SideNote('峰值 ${ms(state.peakRtt)}')),
          const SizedBox(height: 10),
          Text.rich(
            TextSpan(
              children: [
                TextSpan(text: state.sharing ? ms(state.latestRtt) : '—'),
                TextSpan(
                  text: ' ms',
                  style: TextStyle(fontSize: 16, fontWeight: FontWeight.w600, color: c.muted),
                ),
              ],
            ),
            style: const TextStyle(fontSize: 40, fontWeight: FontWeight.w800, height: 1.05, fontFeatures: tabular),
          ),
          const SizedBox(height: 14),
          LatencyBars(values: state.rtt, active: state.sharing),
          const SizedBox(height: 10),
          Text(
            '最近 ${state.rtt.length} 秒平均 ${ms(state.avgRtt)} ms',
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
    Widget chip(String text) => Container(
      padding: const EdgeInsets.symmetric(horizontal: 11, vertical: 5),
      decoration: ShapeDecoration(color: c.card2, shape: const StadiumBorder()),
      child: Text(text, style: const TextStyle(fontSize: 14, fontWeight: FontWeight.w600)),
    );
    return SfCard(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          CardHeader(icon: Icons.content_paste_rounded, title: '剪贴板', trailing: SideNote(timeAgo(state.clipAt))),
          const SizedBox(height: 12),
          Row(
            children: [
              chip('本机'),
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: 8),
                child: Icon(Icons.arrow_forward_rounded, size: 18, color: c.accent),
              ),
              chip(state.peerName),
            ],
          ),
          const SizedBox(height: 12),
          Text(
            state.clipPreview,
            maxLines: 2,
            overflow: TextOverflow.ellipsis,
            style: const TextStyle(fontSize: 15, height: 1.55),
          ),
          const SizedBox(height: 8),
          Text(state.clipMeta, style: TextStyle(fontSize: 14, color: c.muted)),
        ],
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
    final tiles = [
      _QuickTile(
        icon: Icons.content_paste_rounded,
        label: '剪贴板同步',
        on: state.syncClipboard,
        onTap: () => state.update(() => state.syncClipboard = !state.syncClipboard),
      ),
      _QuickTile(
        icon: Icons.keyboard_command_key_rounded,
        label: '⌘ 当作 Ctrl',
        on: state.swapCommandControl,
        onTap: () => state.update(() => state.swapCommandControl = !state.swapCommandControl),
      ),
      _QuickTile(
        icon: Icons.wifi_tethering_rounded,
        label: '局域网广播',
        on: state.announce,
        onTap: () => state.update(() => state.announce = !state.announce),
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
  final VoidCallback onTap;

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
                  Text(on ? '已开启' : '已关闭', style: TextStyle(fontSize: 12.5, color: c.muted)),
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
    return SfCard(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          CardHeader(
            icon: Icons.devices_rounded,
            title: '设备',
            trailing: TextLink(label: '管理', chevron: true, onPressed: () => state.go(SfPage.devices)),
          ),
          const SizedBox(height: 4),
          if (state.paired.isEmpty)
            Padding(
              padding: const EdgeInsets.symmetric(vertical: 12),
              child: Text('还没有配对的设备', style: TextStyle(color: c.muted)),
            ),
          for (var i = 0; i < state.paired.length; i++) ...[
            if (i > 0) Divider(height: 1, thickness: 1, color: c.line),
            Padding(
              padding: const EdgeInsets.symmetric(vertical: 10),
              child: Row(
                children: [
                  IconTile(
                    state.paired[i].laptop ? Icons.laptop_windows_outlined : Icons.desktop_windows_outlined,
                    emphasized: state.paired[i].online,
                  ),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(state.paired[i].name, style: const TextStyle(fontWeight: FontWeight.w600)),
                        Text(
                          state.paired[i].online ? '在线' : state.paired[i].lastSeen,
                          style: TextStyle(fontSize: 13, color: c.muted),
                        ),
                      ],
                    ),
                  ),
                  StatusDot(on: state.paired[i].online),
                ],
              ),
            ),
          ],
          if (state.found.isNotEmpty) ...[
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
                        '局域网里发现 ${state.found.length} 台新设备',
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
                const IconTile(Icons.desktop_mac_outlined, emphasized: true),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text('${state.localName} · 本机', style: const TextStyle(fontWeight: FontWeight.w600)),
                      Text(
                        state.localFingerprint,
                        style: TextStyle(fontSize: 13, color: c.muted, fontFamily: monoFont),
                      ),
                    ],
                  ),
                ),
                SfIconButton(
                  icon: Icons.copy_rounded,
                  tooltip: '复制本机指纹',
                  onPressed: () => copyText(context, state.localFingerprint, '指纹已复制'),
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
            title: '今日',
            trailing: TextLink(label: '日志', chevron: true, onPressed: () => state.go(SfPage.log)),
          ),
          const SizedBox(height: 12),
          Row(
            children: [
              stat('${state.switchesToday}', '次屏幕切换'),
              stat('${state.clipsToday}', '次剪贴板同步'),
              stat('${state.sharedHours}', '小时共享'),
            ],
          ),
          const Spacer(),
          const SizedBox(height: 20),
          HourBars(counts: state.hours),
        ],
      ),
    );
  }
}
