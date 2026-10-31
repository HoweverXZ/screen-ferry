// 主框架：侧栏 + 页面；窗口窄到手机大小时，侧栏换成底部标签栏。
// 切换页面用“淡出再淡入”：旧页面先很快消失，新页面再淡入并轻微上移，两页不会半透明地叠在一起。

import 'package:flutter/material.dart';

import '../ipc/launcher.dart';
import '../pages/about_page.dart';
import '../pages/devices_page.dart';
import '../pages/layout_page.dart';
import '../pages/log_page.dart';
import '../pages/overview_page.dart';
import '../pages/settings_page.dart';
import '../state/app_scope.dart';
import '../state/app_state.dart';
import '../theme/tokens.dart';
import '../widgets/daemon_offline.dart';
import '../widgets/pairing_dialog.dart';
import '../widgets/toast.dart';
import 'bottom_nav.dart';
import 'sidebar.dart';

/// 窗口窄于这个宽度（手机大小）时，侧栏换成底部标签栏。
const phoneWidth = 560.0;

/// 后台的位置只需找一次。
final String? _daemon = daemonLocation();

class HomeShell extends StatelessWidget {
  const HomeShell({super.key});

  static final _pagesKey = GlobalKey();

  Widget _page(SfPage page) => switch (page) {
    SfPage.overview => const OverviewPage(),
    SfPage.devices => const DevicesPage(),
    SfPage.layout => const LayoutPage(),
    SfPage.log => const LogPage(),
    SfPage.settings => const SettingsPage(),
    SfPage.about => const AboutPage(),
  };

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = SfColors.of(context);
    // 页面区放在固定的 GlobalKey 下：窗口在侧栏与底部标签栏两种布局之间切换时，页面状态（滚动位置、筛选）不丢。
    final pages = KeyedSubtree(
      key: _pagesKey,
      child: Stack(
        children: [
          Positioned.fill(
            child: AnimatedSwitcher(
              duration: _FadeThrough.duration,
              transitionBuilder: _FadeThrough.wrap,
              layoutBuilder: (current, previous) => Stack(fit: StackFit.expand, children: [...previous, ?current]),
              // 还没连上过后台时没有数据可显示，先放占位页。
              child: state.daemon == null
                  ? KeyedSubtree(
                      key: const ValueKey('offline'),
                      child: DaemonOffline(daemon: _daemon),
                    )
                  : KeyedSubtree(key: ValueKey(state.page), child: _page(state.page)),
            ),
          ),
          Positioned(
            left: 0,
            right: 0,
            bottom: 28,
            child: Center(child: ToastHost(messages: state.toast)),
          ),
        ],
      ),
    );
    return Scaffold(
      backgroundColor: c.bg,
      body: PairingHost(
        child: LayoutBuilder(
          builder: (context, box) {
            if (box.maxWidth < phoneWidth) {
              return Column(
                children: [
                  Expanded(child: pages),
                  const BottomNav(),
                ],
              );
            }
            return Row(
              children: [
                Sidebar(forceCollapsed: box.maxWidth < 900),
                Expanded(child: pages),
              ],
            );
          },
        ),
      ),
    );
  }
}

/// 进场时动画正向播放，退场时反向播放；按方向用不同的节奏。
class _FadeThrough extends AnimatedWidget {
  const _FadeThrough({required Animation<double> animation, required this.child}) : super(listenable: animation);

  static const duration = Duration(milliseconds: 380);

  /// 用静态函数而不是闭包：每秒刷新数据时，AnimatedSwitcher 不会因为“换了构建函数”而重建转场。
  static Widget wrap(Widget child, Animation<double> animation) => _FadeThrough(animation: animation, child: child);

  /// 旧页面在前 30% 的时间里淡出；新页面从 30% 开始淡入。
  static const _out = Interval(0.7, 1, curve: Curves.easeIn);
  static const _in = Interval(0.3, 1, curve: Curves.easeOut);
  static const _rise = Interval(0.3, 1, curve: Motion.ease);

  final Widget child;

  @override
  Widget build(BuildContext context) {
    final animation = listenable as Animation<double>;
    final t = animation.value;
    if (animation.status == AnimationStatus.reverse) {
      return IgnorePointer(
        child: Opacity(opacity: _out.transform(t), child: child),
      );
    }
    return Opacity(
      opacity: _in.transform(t),
      child: Transform.translate(offset: Offset(0, 14 * (1 - _rise.transform(t))), child: child),
    );
  }
}
