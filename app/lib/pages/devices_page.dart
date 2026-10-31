// 设备：本机横幅、已配对设备的卡片墙、局域网里发现但还没配对的设备（虚线框）。

import 'dart:math';

import 'package:flutter/material.dart';

import '../state/app_scope.dart';
import '../state/app_state.dart';
import '../theme/tokens.dart';
import '../widgets/page_scaffold.dart';
import '../widgets/surface.dart';

class DevicesPage extends StatelessWidget {
  const DevicesPage({super.key});

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = SfColors.of(context);
    return PageScaffold(
      title: '设备',
      children: [
        Container(
          padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 22),
          decoration: ShapeDecoration(color: c.accentSoft, shape: continuous(26)),
          child: BannerRow(
            leading: Container(
              width: 64,
              height: 64,
              decoration: ShapeDecoration(color: c.accent, shape: continuous(22)),
              child: Icon(
                state.isServer ? Icons.desktop_mac_outlined : Icons.desktop_windows_outlined,
                size: 32,
                color: c.onAccent,
              ),
            ),
            body: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(
                  state.localName,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: const TextStyle(fontSize: 22, fontWeight: FontWeight.w700),
                ),
                Text.rich(
                  TextSpan(
                    text: '本机 · 指纹 ',
                    children: [
                      TextSpan(
                        text: state.localFingerprint,
                        style: const TextStyle(fontFamily: monoFont, fontFamilyFallback: monoFallback),
                      ),
                    ],
                  ),
                  style: TextStyle(color: c.muted),
                ),
              ],
            ),
            trailing: PillButton(
              label: '复制指纹',
              icon: Icons.copy_rounded,
              onPressed: () => copyText(context, state.localFingerprint, '指纹已复制'),
            ),
          ),
        ),
        SectionTitle('已配对', note: '${state.peers.length} 台'),
        _CardWall(
          empty: const _Empty(
            icon: Icons.link_rounded,
            title: '还没有配对的设备',
            text: '在另一台电脑上运行 ScreenFerry，两边核对配对码后会出现在这里。',
          ),
          children: [for (final d in state.peers) _PairedCard(key: ValueKey('p-${d.fingerprint}'), device: d)],
        ),
        const SectionTitle('在局域网里发现', note: '还没配对'),
        if (state.isServer)
          const _Empty(
            icon: Icons.radar_rounded,
            title: '等另一台电脑来找本机',
            text: '在另一台电脑上运行 ScreenFerry 并选择本机后，这里会弹出配对码请你核对。',
          )
        else
          _CardWall(
            empty: const _Empty(icon: Icons.radar_rounded, title: '局域网里没有其他设备', text: '运行 ScreenFerry 的电脑会自动出现在这里。'),
            children: [for (final d in state.found) _FoundCard(key: ValueKey('f-${d.fingerprint}'), device: d)],
          ),
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 6),
          child: Text(
            '连接全程 TLS 1.3 加密。首次连接时两台电脑会显示同一个 6 位配对码，核对一致后才会互相信任；移除后需要重新配对。',
            style: TextStyle(fontSize: 13, color: c.muted, height: 1.6),
          ),
        ),
      ],
    );
  }
}

/// 卡片墙：每张卡片至少 250 宽，放得下几列就几列。
class _CardWall extends StatelessWidget {
  const _CardWall({required this.children, required this.empty});

  final List<Widget> children;
  final Widget empty;

  @override
  Widget build(BuildContext context) {
    if (children.isEmpty) return empty;
    return LayoutBuilder(
      builder: (context, box) {
        const gap = 16.0;
        final columns = max(1, ((box.maxWidth + gap) / (250 + gap)).floor());
        final width = (box.maxWidth - gap * (columns - 1)) / columns;
        return Wrap(
          spacing: gap,
          runSpacing: gap,
          children: [for (final child in children) SizedBox(width: width, height: 220, child: child)],
        );
      },
    );
  }
}

class _Empty extends StatelessWidget {
  const _Empty({required this.icon, required this.title, required this.text});

  final IconData icon;
  final String title;
  final String text;

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    return DashedBox(
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 22),
        child: Row(
          children: [
            Icon(icon, size: 28, color: c.accent),
            const SizedBox(width: 16),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(title, style: const TextStyle(fontWeight: FontWeight.w600)),
                  Text(text, style: TextStyle(color: c.muted)),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _PairedCard extends StatefulWidget {
  const _PairedCard({super.key, required this.device});

  final PeerInfo device;

  @override
  State<_PairedCard> createState() => _PairedCardState();
}

class _PairedCardState extends State<_PairedCard> {
  bool _confirming = false;
  bool _leaving = false;
  late final bool _fresh = AppScope.read(context).isFresh(widget.device.fingerprint);

  void _remove() {
    setState(() => _leaving = true);
    final state = AppScope.read(context);
    Future<void>.delayed(const Duration(milliseconds: 320), () => state.forget(widget.device));
  }

  @override
  Widget build(BuildContext context) {
    final d = widget.device;
    final c = SfColors.of(context);
    final state = AppScope.of(context);
    final addr = state.peer?.fingerprint == d.fingerprint ? state.peer?.addr ?? '' : '';
    final seen = d.online && addr.isNotEmpty ? '在线 · $addr' : seenText(d);
    Widget card = SfCard(
      padding: const EdgeInsets.all(20),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              IconTile(
                state.isServer ? Icons.desktop_windows_outlined : Icons.desktop_mac_outlined,
                size: 52,
                radius: 18,
                emphasized: d.online,
              ),
              Pill(d.online ? '在线' : '离线', tone: d.online ? PillTone.good : PillTone.plain),
            ],
          ),
          const SizedBox(height: 12),
          Text(
            d.name,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: const TextStyle(fontSize: 19, fontWeight: FontWeight.w700),
          ),
          const SizedBox(height: 2),
          Text.rich(
            TextSpan(
              text: '指纹 ',
              children: [
                TextSpan(
                  text: d.short,
                  style: const TextStyle(fontFamily: monoFont, fontFamilyFallback: monoFallback),
                ),
              ],
            ),
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: TextStyle(fontSize: 14, color: c.muted),
          ),
          const Spacer(),
          Divider(height: 1, thickness: 1, color: c.line),
          const SizedBox(height: 10),
          SizedBox(
            height: 34,
            child: AnimatedSwitcher(
              duration: Motion.fast,
              child: _confirming
                  ? Row(
                      key: const ValueKey('confirm'),
                      children: [
                        Expanded(
                          child: Text('移除后需重新配对', style: TextStyle(fontSize: 13, color: c.muted)),
                        ),
                        SmallButton(label: '取消', onPressed: () => setState(() => _confirming = false)),
                        const SizedBox(width: 6),
                        SmallButton(label: '移除', danger: true, onPressed: state.online ? _remove : null),
                      ],
                    )
                  : Row(
                      key: const ValueKey('normal'),
                      children: [
                        Expanded(
                          child: Text(seen, style: TextStyle(fontSize: 13, color: c.muted)),
                        ),
                        TextLink(
                          label: '移除',
                          danger: true,
                          onPressed: state.online ? () => setState(() => _confirming = true) : null,
                        ),
                      ],
                    ),
            ),
          ),
        ],
      ),
    );
    card = AnimatedScale(
      scale: _leaving ? 0.9 : 1,
      duration: Motion.medium,
      curve: Motion.ease,
      child: AnimatedOpacity(opacity: _leaving ? 0 : 1, duration: Motion.fast, child: card),
    );
    if (_fresh) {
      card = TweenAnimationBuilder<double>(
        tween: Tween(begin: 0, end: 1),
        duration: const Duration(milliseconds: 550),
        curve: Motion.spring,
        builder: (context, t, child) => Opacity(
          opacity: t.clamp(0, 1),
          child: Transform.scale(scale: 0.85 + 0.15 * t, child: child),
        ),
        child: card,
      );
    }
    return card;
  }
}

class _FoundCard extends StatelessWidget {
  const _FoundCard({super.key, required this.device});

  final FoundDevice device;

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    final state = AppScope.of(context);
    return DashedBox(
      child: Padding(
        padding: const EdgeInsets.all(20),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [IconTile(Icons.desktop_mac_outlined, size: 52, radius: 18), Pill('未配对')],
            ),
            const SizedBox(height: 12),
            Text(
              device.name,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: const TextStyle(fontSize: 19, fontWeight: FontWeight.w700),
            ),
            const SizedBox(height: 2),
            Text(
              device.addr.isEmpty ? '指纹 ${device.short}' : device.addr,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(fontSize: 14, color: c.muted),
            ),
            const Spacer(),
            Divider(height: 1, thickness: 1, color: c.line),
            const SizedBox(height: 10),
            Row(
              children: [
                Expanded(
                  child: Text('正在广播', style: TextStyle(fontSize: 13, color: c.muted)),
                ),
                SmallButton(
                  label: '配对',
                  tone: PillTone.accent,
                  onPressed: state.online ? () => state.connectTo(device) : null,
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}
