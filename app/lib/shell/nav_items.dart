// 导航项：侧栏和窄窗口底部的标签栏共用。

import '../l10n/l10n.dart';
import '../state/app_state.dart';
import '../widgets/glyph.dart';

class NavItem {
  const NavItem(this.page, this.glyph);

  final SfPage page;
  final Glyph glyph;

  String label(AppLocalizations l) => switch (page) {
    SfPage.overview => l.navOverview,
    SfPage.devices => l.navDevices,
    SfPage.layout => l.navLayout,
    SfPage.log => l.navLog,
    SfPage.settings || SfPage.about => l.navSettings,
  };
}

const navItems = [
  NavItem(SfPage.overview, Glyph.dashboard),
  NavItem(SfPage.devices, Glyph.devices),
  NavItem(SfPage.layout, Glyph.layout),
  NavItem(SfPage.log, Glyph.log),
  NavItem(SfPage.settings, Glyph.settings),
];

/// 当前页面对应第几个导航项。“关于”从“设置”进入，归在“设置”下。
int navIndexOf(SfPage page) {
  final nav = page == SfPage.about ? SfPage.settings : page;
  return navItems.indexWhere((i) => i.page == nav);
}
