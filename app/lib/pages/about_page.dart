import 'package:flutter/material.dart';

import '../l10n/l10n.dart';
import '../state/app_scope.dart';
import '../state/app_state.dart';
import '../theme/tokens.dart';
import '../widgets/brand.dart';
import '../widgets/controls.dart';
import '../widgets/group.dart';
import '../widgets/page_scaffold.dart';
import '../widgets/pressable.dart';
import '../widgets/surface.dart';
import 'update_panel.dart';

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
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Row(
                children: [
                  const AppIconMark(size: 66),
                  const SizedBox(width: 18),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        const Text('ScreenFerry', style: TextStyle(fontSize: 24, fontWeight: FontWeight.w800)),
                        const SizedBox(height: 2),
                        Text(
                          state.version.isEmpty
                              ? context.l10n.aboutTagline
                              : context.l10n.aboutVersionLine(state.version),
                          style: TextStyle(color: c.muted, height: 1.5),
                        ),
                      ],
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 18),
              const UpdatePanel(),
            ],
          ),
        ),
        SfGroup(
          children: [
            SfRow(
              icon: Icons.update_rounded,
              title: context.l10n.updateAuto,
              subtitle: switch (state.updateInfo.checkedAt) {
                final at? =>
                  '${context.l10n.updateAutoHint} · ${context.l10n.updateLastChecked(timeAgo(context.l10n, at))}',
                null => context.l10n.updateAutoHint,
              },
              trailing: SfSwitch(
                value: state.settings.autoUpdate,
                label: context.l10n.updateAuto,
                onChanged: state.online ? (v) => state.updateSettings(SettingsPatch(autoUpdate: v)) : null,
              ),
            ),
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
