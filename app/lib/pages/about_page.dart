import 'package:flutter/material.dart';

import '../state/app_scope.dart';
import '../state/app_state.dart';
import '../theme/tokens.dart';
import '../widgets/group.dart';
import '../widgets/page_scaffold.dart';
import '../widgets/pressable.dart';
import '../widgets/surface.dart';

class AboutPage extends StatelessWidget {
  const AboutPage({super.key});

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = SfColors.of(context);
    return PageScaffold(
      title: '关于',
      leading: Pressable(
        onTap: () => state.go(SfPage.settings),
        color: c.card,
        hoverColor: c.card2,
        radius: 22,
        tooltip: '返回设置',
        semanticLabel: '返回设置',
        child: const SizedBox.square(dimension: 44, child: Icon(Icons.arrow_back_ios_new_rounded, size: 18)),
      ),
      children: [
        SfCard(
          child: BannerRow(
            gap: 22,
            leading: Container(
              width: 88,
              height: 88,
              decoration: ShapeDecoration(color: c.accent, shape: continuous(26)),
              child: Center(child: FerryGlyph(size: 50, color: c.onAccent)),
            ),
            body: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                const Wrap(
                  spacing: 10,
                  runSpacing: 6,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: [
                    Text('ScreenFerry', style: TextStyle(fontSize: 24, fontWeight: FontWeight.w800)),
                    Pill('v0.1.0', tone: PillTone.accent),
                  ],
                ),
                const SizedBox(height: 6),
                Text('在 Mac 与 Windows 之间共享一套键盘鼠标，同步剪贴板。全程加密，配对后才互相信任。', style: TextStyle(color: c.muted, height: 1.5)),
              ],
            ),
            trailing: PillButton(
              label: '检查更新',
              icon: Icons.refresh_rounded,
              soft: true,
              onPressed: () => state.showToast('已是最新版本'),
            ),
          ),
        ),
        SfGroup(
          children: [
            SfRow(
              icon: Icons.code_rounded,
              title: '作者主页',
              subtitle: 'github.com/icloudza',
              trailing: Icon(Icons.copy_rounded, size: 18, color: c.muted),
              onTap: () => copyText(context, 'https://github.com/icloudza', '链接已复制'),
            ),
            const SfRow(icon: Icons.info_outline_rounded, title: '许可证', subtitle: 'MIT OR Apache-2.0'),
            const SfRow(icon: Icons.lock_rounded, title: '加密与配对', subtitle: 'TLS 1.3 · 首次连接核对 6 位配对码'),
            const SfRow(icon: Icons.keyboard_rounded, title: '协议版本', subtitle: '1 · 键鼠与剪贴板各走一条连接'),
          ],
        ),
      ],
    );
  }
}
