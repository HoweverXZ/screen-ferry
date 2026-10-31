// 设置：外观、键盘与剪贴板、通用，以及进入“关于”。

import 'package:flutter/material.dart';

import '../state/app_scope.dart';
import '../state/app_state.dart';
import '../theme/tokens.dart';
import '../widgets/controls.dart';
import '../widgets/group.dart';
import '../widgets/page_scaffold.dart';
import '../widgets/pressable.dart';
import '../widgets/surface.dart';

class SettingsPage extends StatelessWidget {
  const SettingsPage({super.key});

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = SfColors.of(context);
    return PageScaffold(
      title: '设置',
      children: [
        const SectionTitle('外观'),
        ListenableBuilder(
          listenable: state.appearance,
          builder: (context, _) => SfGroup(
            children: [
              SfRow(
                icon: Icons.contrast_rounded,
                title: '主题',
                trailing: SfSegmented<ThemeMode>(
                  label: '主题',
                  segments: const [(ThemeMode.system, '跟随系统'), (ThemeMode.light, '浅色'), (ThemeMode.dark, '深色')],
                  value: state.appearance.mode,
                  onChanged: (v) => state.appearance.mode = v,
                ),
              ),
              SfRow(
                icon: Icons.palette_outlined,
                title: '强调色',
                subtitle: '开关、选中项和按钮的颜色',
                trailing: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    for (var i = 0; i < AccentChoice.all.length; i++) ...[
                      if (i > 0) const SizedBox(width: 10),
                      _Swatch(
                        choice: AccentChoice.all[i],
                        selected: state.appearance.accent == i,
                        onTap: () => state.appearance.accent = i,
                      ),
                    ],
                  ],
                ),
              ),
            ],
          ),
        ),
        const SectionTitle('键盘与剪贴板'),
        SfGroup(
          children: [
            SfRow(
              icon: Icons.keyboard_command_key_rounded,
              title: '⌘ 当作 Ctrl',
              subtitle: '在 Windows 上按 ⌘C、⌘V 就是复制、粘贴',
              trailing: SfSwitch(
                value: state.swapCommandControl,
                label: '⌘ 当作 Ctrl',
                onChanged: (v) => state.update(() => state.swapCommandControl = v),
              ),
            ),
            SfRow(
              icon: Icons.content_paste_rounded,
              title: '同步剪贴板',
              subtitle: '光标切过去时，对方才来取最新的内容',
              trailing: SfSwitch(
                value: state.syncClipboard,
                label: '同步剪贴板',
                onChanged: (v) => state.update(() => state.syncClipboard = v),
              ),
            ),
            SfRow(
              icon: Icons.storage_rounded,
              title: '单项大小上限',
              subtitle: '超过的内容不同步',
              trailing: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  SizedBox(
                    width: 180,
                    child: Slider(
                      value: state.clipboardLimitMb,
                      min: 1,
                      max: 64,
                      divisions: 63,
                      label: '${state.clipboardLimitMb.round()} MB',
                      onChanged: (v) => state.update(() => state.clipboardLimitMb = v),
                    ),
                  ),
                  SizedBox(
                    width: 56,
                    child: Text(
                      '${state.clipboardLimitMb.round()} MB',
                      textAlign: TextAlign.end,
                      style: const TextStyle(fontFeatures: tabular),
                    ),
                  ),
                ],
              ),
            ),
          ],
        ),
        const SectionTitle('通用'),
        SfGroup(
          children: [
            SfRow(
              icon: Icons.power_settings_new_rounded,
              title: '开机自动启动',
              trailing: SfSwitch(
                value: state.launchAtLogin,
                label: '开机自动启动',
                onChanged: (v) => state.update(() => state.launchAtLogin = v),
              ),
            ),
            SfRow(
              icon: Icons.wifi_tethering_rounded,
              title: '在局域网里广播本机',
              subtitle: '对方运行 connect 时不用填地址',
              trailing: SfSwitch(
                value: state.announce,
                label: '在局域网里广播本机',
                onChanged: (v) => state.update(() => state.announce = v),
              ),
            ),
            SfRow(icon: Icons.folder_outlined, title: '数据目录', subtitle: state.dataDir, mono: true),
            SfRow(
              icon: Icons.info_outline_rounded,
              title: '关于 ScreenFerry',
              subtitle: '版本 0.1.0',
              trailing: Icon(Icons.chevron_right_rounded, color: c.muted),
              onTap: () => state.go(SfPage.about),
            ),
          ],
        ),
      ],
    );
  }
}

class _Swatch extends StatelessWidget {
  const _Swatch({required this.choice, required this.selected, required this.onTap});

  final AccentChoice choice;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    return Pressable(
      onTap: onTap,
      radius: 999,
      pressScale: 0.9,
      selected: selected,
      semanticLabel: '${choice.name}色',
      tooltip: '${choice.name}色',
      child: AnimatedContainer(
        duration: Motion.fast,
        width: 40,
        height: 40,
        padding: const EdgeInsets.all(5),
        decoration: BoxDecoration(
          shape: BoxShape.circle,
          border: Border.all(color: c.fg.withValues(alpha: selected ? 1 : 0), width: 2.5),
        ),
        child: DecoratedBox(
          decoration: BoxDecoration(shape: BoxShape.circle, color: choice.light),
        ),
      ),
    );
  }
}
