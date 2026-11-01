// 设置：外观、鼠标与剪贴板、通用，以及进入“关于”。按键映射、滚动方向等在“设备”里逐台设置。

import 'package:flutter/material.dart';

import '../state/app_scope.dart';
import '../state/app_state.dart';
import '../theme/tokens.dart';
import '../widgets/controls.dart';
import '../widgets/dpi_field.dart';
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
    final s = state.settings;
    // 没连上后台时开关不可点（返回 null）。
    ValueChanged<bool>? set(SettingsPatch Function(bool) patch) =>
        state.online ? (v) => state.updateSettings(patch(v)) : null;
    return PageScaffold(
      title: '设置',
      children: [
        const SectionTitle('这台电脑'),
        SfGroup(
          children: [
            SfRow(
              icon: Icons.swap_horiz_rounded,
              title: '角色',
              subtitle: state.isServer ? '键盘鼠标接在这台上，用它控制别的电脑' : '由另一台电脑的键盘鼠标控制 · 切换后重新连接',
              trailing: SfSegmented<Role>(
                label: '角色',
                segments: const [(Role.server, '控制别的电脑'), (Role.client, '被控制')],
                value: state.role,
                onChanged: state.online ? state.setRole : null,
              ),
            ),
          ],
        ),
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
                  onChanged: state.online ? state.setTheme : null,
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
                        onTap: state.online ? () => state.setAccent(i) : null,
                      ),
                    ],
                  ],
                ),
              ),
            ],
          ),
        ),
        const SectionTitle('鼠标与剪贴板'),
        SfGroup(
          children: [
            if (state.isServer) ...[
              SfRow(
                icon: Icons.mouse_outlined,
                title: '本机鼠标的 DPI',
                subtitle: '每台设备的光标速度以它为准',
                trailing: DpiField(
                  value: s.mouseDpi,
                  label: '本机鼠标的 DPI',
                  min: DaemonSettings.mouseDpiRange.min,
                  max: DaemonSettings.mouseDpiRange.max,
                  onChanged: state.online ? (v) => state.updateSettings(SettingsPatch(mouseDpi: v)) : null,
                ),
              ),
              SfRow(
                icon: Icons.devices_rounded,
                title: '按键映射、滚动方向',
                subtitle: '在“设备”里逐台设置',
                trailing: Icon(Icons.chevron_right_rounded, color: c.muted),
                onTap: () => state.go(SfPage.devices),
              ),
            ],
            SfRow(
              icon: Icons.content_paste_rounded,
              title: '同步剪贴板',
              subtitle: '光标切过去时，对方才来取最新的内容',
              trailing: SfSwitch(
                value: s.clipboard,
                label: '同步剪贴板',
                onChanged: set((v) => SettingsPatch(clipboard: v)),
              ),
            ),
            SfRow(
              icon: Icons.storage_rounded,
              title: '单项大小上限',
              subtitle: '图片、文字等每一项超过这么大就不同步',
              trailing: _LimitSlider(
                value: s.clipboardLimitMb,
                onChanged: state.online ? (mb) => state.updateSettings(SettingsPatch(clipboardLimitMb: mb)) : null,
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
                value: s.autostart,
                label: '开机自动启动',
                onChanged: set((v) => SettingsPatch(autostart: v)),
              ),
            ),
            if (state.isServer)
              SfRow(
                icon: Icons.wifi_tethering_rounded,
                title: '在局域网里广播本机',
                subtitle: '另一台电脑不用填地址就能找到本机',
                trailing: SfSwitch(
                  value: s.announce,
                  label: '在局域网里广播本机',
                  onChanged: set((v) => SettingsPatch(announce: v)),
                ),
              ),
            SfRow(icon: Icons.folder_outlined, title: '数据目录', subtitle: state.dataDir, mono: true),
            SfRow(
              icon: Icons.info_outline_rounded,
              title: '关于 ScreenFerry',
              subtitle: state.version.isEmpty ? null : '版本 ${state.version}',
              trailing: Icon(Icons.chevron_right_rounded, color: c.muted),
              onTap: () => state.go(SfPage.about),
            ),
          ],
        ),
      ],
    );
  }
}

/// 剪贴板单项上限的滑块：拖动时只改显示，松手才发给后台。
class _LimitSlider extends StatefulWidget {
  const _LimitSlider({required this.value, required this.onChanged});

  final int value;
  final ValueChanged<int>? onChanged;

  @override
  State<_LimitSlider> createState() => _LimitSliderState();
}

class _LimitSliderState extends State<_LimitSlider> {
  double? _dragging;

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    const range = DaemonSettings.clipboardLimitRange;
    final onChanged = widget.onChanged;
    final shown = (_dragging ?? widget.value.toDouble()).clamp(range.min.toDouble(), range.max.toDouble());
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        SizedBox(
          width: 180,
          child: Slider(
            value: shown,
            min: range.min.toDouble(),
            max: range.max.toDouble(),
            divisions: range.max - range.min,
            semanticFormatterCallback: (v) => '${v.round()} MB',
            onChanged: onChanged == null ? null : (v) => setState(() => _dragging = v),
            onChangeEnd: onChanged == null
                ? null
                : (v) {
                    setState(() => _dragging = null);
                    if (v.round() != widget.value) onChanged(v.round());
                  },
          ),
        ),
        SizedBox(
          width: 56,
          child: Text(
            '${shown.round()} MB',
            textAlign: TextAlign.end,
            style: TextStyle(fontFeatures: tabular, color: c.muted),
          ),
        ),
      ],
    );
  }
}

class _Swatch extends StatelessWidget {
  const _Swatch({required this.choice, required this.selected, required this.onTap});

  final AccentChoice choice;
  final bool selected;
  final VoidCallback? onTap;

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
