import 'package:flutter/material.dart';

import '../l10n/l10n.dart';
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
      title: context.l10n.aboutTitle,
      leading: Pressable(
        onTap: () => state.go(SfPage.settings),
        color: c.card,
        hoverColor: c.card2,
        radius: 22,
        tooltip: context.l10n.aboutBack,
        semanticLabel: context.l10n.aboutBack,
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
                Wrap(
                  spacing: 10,
                  runSpacing: 6,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: [
                    const Text('ScreenFerry', style: TextStyle(fontSize: 24, fontWeight: FontWeight.w800)),
                    if (state.version.isNotEmpty) Pill('v${state.version}', tone: PillTone.accent),
                  ],
                ),
                const SizedBox(height: 6),
                Text(context.l10n.aboutTagline, style: TextStyle(color: c.muted, height: 1.5)),
              ],
            ),
            trailing: PillButton(
              label: context.l10n.aboutCheckUpdates,
              icon: Icons.refresh_rounded,
              soft: true,
              onPressed: () => state.showToast(context.l10n.aboutUpdatesSoon),
            ),
          ),
        ),
        SfGroup(
          children: [
            SfRow(
              icon: Icons.code_rounded,
              title: context.l10n.aboutAuthor,
              subtitle: 'github.com/icloudza',
              trailing: Icon(Icons.open_in_new_rounded, size: 18, color: c.muted),
              onTap: () => state.openLink('https://github.com/icloudza'),
            ),
            SfRow(icon: Icons.info_outline_rounded, title: context.l10n.aboutLicense, subtitle: 'MIT OR Apache-2.0'),
          ],
        ),
      ],
    );
  }
}
