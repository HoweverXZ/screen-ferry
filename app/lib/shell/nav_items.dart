// 导航项：侧栏和窄窗口底部的标签栏共用。

import '../state/app_state.dart';
import '../widgets/glyph.dart';

class NavItem {
  const NavItem(this.page, this.glyph, this.label);

  final SfPage page;
  final Glyph glyph;
  final String label;
}

const navItems = [
  NavItem(SfPage.overview, Glyph.dashboard, '仪表盘'),
  NavItem(SfPage.devices, Glyph.devices, '设备'),
  NavItem(SfPage.layout, Glyph.layout, '布局'),
  NavItem(SfPage.log, Glyph.log, '日志'),
  NavItem(SfPage.settings, Glyph.settings, '设置'),
];

/// 当前页面对应第几个导航项。“关于”从“设置”进入，归在“设置”下。
int navIndexOf(SfPage page) {
  final nav = page == SfPage.about ? SfPage.settings : page;
  return navItems.indexWhere((i) => i.page == nav);
}
