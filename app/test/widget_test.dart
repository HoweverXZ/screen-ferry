import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:screenferry_settings/app.dart';
import 'package:screenferry_settings/shell/bottom_nav.dart';
import 'package:screenferry_settings/shell/sidebar.dart';
import 'package:screenferry_settings/state/app_state.dart';
import 'package:screenferry_settings/theme/tokens.dart';
import 'package:screenferry_settings/widgets/glyph.dart';
import 'package:screenferry_settings/widgets/page_scaffold.dart';
import 'package:screenferry_settings/widgets/surface.dart';

/// 花朵一直在转，等不到“全部动画停下”，所以按固定时长推进。
Future<void> settle(WidgetTester tester, {int steps = 6}) async {
  for (var i = 0; i < steps; i++) {
    await tester.pump(const Duration(milliseconds: 150));
  }
}

/// 用演示后台启动界面。默认不让演示数据自己跳动，测试结果才稳定。
Future<AppState> launch(WidgetTester tester, Size size, {Backend? backend}) async {
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  final state = AppState(backend: backend ?? DemoBackend(animate: false));
  addTearDown(state.dispose);
  await tester.pumpWidget(ScreenFerryApp(state: state));
  state.start();
  await settle(tester);
  return state;
}

Future<void> visitAll(WidgetTester tester, AppState state) async {
  for (final page in [SfPage.devices, SfPage.layout, SfPage.log, SfPage.settings, SfPage.about, SfPage.overview]) {
    state.go(page);
    await settle(tester);
  }
}

Finder navItem(String label) => find.descendant(of: find.byType(Sidebar), matching: find.text(label));

/// 布局页里本机（`local`）或某台设备（证书指纹）的框。
Rect screenRect(WidgetTester tester, String id) => tester.getRect(find.byKey(ValueKey('screen-$id')));

String fingerprintOf(AppState state, String name) => state.peers.firstWhere((p) => p.name == name).fingerprint;

/// 打开布局页，返回演示数据里三台设备的指纹：办公室 PC、书房 PC（都在右侧）、家里的笔记本（左侧）。
Future<(String, String, String)> openLayout(WidgetTester tester, AppState state) async {
  state.go(SfPage.layout);
  await settle(tester);
  return (fingerprintOf(state, '办公室 PC'), fingerprintOf(state, '书房 PC'), fingerprintOf(state, '家里的笔记本'));
}

/// 页面大标题所在标题栏的底色。
Color headerColor(WidgetTester tester, String title) {
  final text = find.descendant(of: find.byType(CustomScrollView), matching: find.text(title)).first;
  return tester.widget<Container>(find.ancestor(of: text, matching: find.byType(Container)).first).color!;
}

void main() {
  for (final (name, size) in [
    ('默认窗口', const Size(1160, 780)),
    ('最小窗口', const Size(880, 600)),
    ('窄屏', const Size(600, 900)),
    ('手机大小', const Size(375, 667)),
  ]) {
    testWidgets('$name下所有页面都能正常显示', (tester) async {
      final state = await launch(tester, size);
      expect(find.text('仪表盘'), findsWidgets);
      await visitAll(tester, state);
      expect(find.text('往返延迟'), findsOneWidget);
    });
  }

  testWidgets('点花朵中心暂停与继续共享', (tester) async {
    final state = await launch(tester, const Size(1160, 780));
    await tester.tap(find.byTooltip('暂停共享'));
    await settle(tester);
    expect(state.sharing, isFalse);
    expect(find.text('已暂停'), findsOneWidget);

    await tester.tap(find.byTooltip('继续共享'));
    await settle(tester);
    expect(state.sharing, isTrue);
  });

  testWidgets('客户端配对局域网里发现的服务端', (tester) async {
    final backend = DemoBackend(role: Role.client, animate: false);
    final state = await launch(tester, const Size(1160, 780), backend: backend);
    state.go(SfPage.devices);
    await settle(tester);

    await tester.tap(find.text('配对'));
    await settle(tester);
    expect(backend.sent.whereType<ConnectRequest>(), hasLength(1));
    expect(find.text('与“会议室 Mac”配对'), findsOneWidget);

    await tester.tap(find.text('数字一致，配对'));
    await settle(tester, steps: 12);
    expect(backend.sent.whereType<PairReplyRequest>().single.accept, isTrue);
    expect(state.peers.first.name, '会议室 Mac');
    expect(state.found, isEmpty);
    expect(find.text('与“会议室 Mac”配对'), findsNothing, reason: '配对结束后对话框自动关闭');
    expect(find.text('局域网里没有其他设备'), findsOneWidget);
  });

  testWidgets('服务端不列出局域网设备，只说明怎么配对', (tester) async {
    final state = await launch(tester, const Size(1160, 780));
    state.go(SfPage.devices);
    await settle(tester);
    expect(find.text('等另一台电脑来找本机'), findsOneWidget);
    expect(find.text('配对'), findsNothing);
  });

  testWidgets('移除设备前要在卡片上确认', (tester) async {
    final state = await launch(tester, const Size(1160, 780));
    state.go(SfPage.devices);
    await settle(tester);

    await tester.tap(find.text('移除').first);
    await settle(tester);
    expect(find.text('移除后需重新配对'), findsOneWidget);
    expect(state.peers, hasLength(3), reason: '确认前不能移除');

    // 第一张卡片底部现在是“取消 / 移除”，它的“移除”仍是第一个。
    await tester.tap(find.text('移除').first);
    await settle(tester);
    expect(state.peers.map((d) => d.name), isNot(contains('办公室 PC')));
  });

  testWidgets('日志可以筛选和清空', (tester) async {
    final state = await launch(tester, const Size(1160, 780));
    state.go(SfPage.log);
    await settle(tester);

    final total = state.log.length;
    expect(find.text('共 $total 条'), findsOneWidget);
    await tester.tap(find.text('剪贴板').first);
    await settle(tester);
    final clips = state.log.where((e) => e.kind == LogKind.clip).length;
    expect(find.text('共 $clips 条'), findsOneWidget);

    await tester.tap(find.text('清空'));
    await settle(tester);
    expect(find.text('暂无记录'), findsOneWidget);
  });

  testWidgets('切换深色主题和强调色', (tester) async {
    final state = await launch(tester, const Size(1160, 780));
    state.go(SfPage.settings);
    await settle(tester);

    await tester.tap(find.text('深色'));
    await settle(tester);
    expect(state.appearance.mode, ThemeMode.dark);

    await tester.tap(find.byTooltip('紫色'));
    await settle(tester);
    expect(state.appearance.accent, 2);
  });

  testWidgets('点侧栏菜单切换页面', (tester) async {
    final state = await launch(tester, const Size(1160, 780));
    for (final (label, page) in [
      ('设备', SfPage.devices),
      ('布局', SfPage.layout),
      ('日志', SfPage.log),
      ('设置', SfPage.settings),
      ('仪表盘', SfPage.overview),
    ]) {
      await tester.tap(navItem(label));
      await settle(tester);
      expect(state.page, page);
    }
  });

  testWidgets('切换主题后标题栏跟着换色', (tester) async {
    final state = await launch(tester, const Size(1160, 780));
    state.go(SfPage.settings);
    await settle(tester);
    expect(headerColor(tester, '设置'), SfColors.light.bg);

    await tester.tap(find.text('深色'));
    await settle(tester);
    expect(headerColor(tester, '设置'), SfColors.dark.bg);

    await tester.tap(find.text('浅色'));
    await settle(tester);
    expect(headerColor(tester, '设置'), SfColors.light.bg);
  });

  testWidgets('切换页面时新旧两页不会同时可见', (tester) async {
    final state = await launch(tester, const Size(1160, 780));
    state.go(SfPage.devices);
    var sawOld = false;
    var sawNew = false;
    for (var ms = 0; ms < 420; ms += 20) {
      await tester.pump(const Duration(milliseconds: 20));
      final pages = find.byType(PageScaffold).evaluate().toList();
      final visible = [
        for (final page in pages)
          tester
              .widget<Opacity>(
                find.ancestor(of: find.byElementPredicate((e) => e == page), matching: find.byType(Opacity)).first,
              )
              .opacity,
      ].where((o) => o > 0.01).length;
      expect(visible, lessThanOrEqualTo(1), reason: '第 $ms 毫秒有 $visible 页同时可见');
      if (pages.length == 2) sawOld = true;
      if (pages.length == 1) sawNew = true;
    }
    expect(sawOld && sawNew, isTrue);
  });

  testWidgets('鼠标移开菜单项时底色不会先变暗', (tester) async {
    await launch(tester, const Size(1160, 780));
    // 悬停色本身比侧栏底色略深；渐变过程中不能比悬停色更暗。
    final side = SfColors.light.side;
    final floor = SfColors.light.navHover.computeLuminance() - 0.005;
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer(location: Offset.zero);
    addTearDown(mouse.removePointer);

    final item = navItem('布局');
    final box = find.ancestor(of: item, matching: find.byType(AnimatedContainer)).first;
    Color fill() {
      final deco = tester.widget<DecoratedBox>(find.descendant(of: box, matching: find.byType(DecoratedBox)).first);
      return Color.alphaBlend((deco.decoration as ShapeDecoration).color!, side);
    }

    var sawHover = false;
    for (final target in [tester.getCenter(item), const Offset(700, 400)]) {
      await mouse.moveTo(target);
      for (var i = 0; i < 12; i++) {
        await tester.pump(const Duration(milliseconds: 20));
        if (fill() != side) sawHover = true;
        expect(fill().computeLuminance(), greaterThan(floor));
      }
    }
    // 确认真的出现过悬停色，测试不是空跑。
    expect(sawHover, isTrue);
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));

  for (final (name, size, wide) in [('宽窗口', const Size(1160, 780), true), ('窄屏', const Size(600, 900), false)]) {
    testWidgets('$name下横幅按钮的位置', (tester) async {
      final state = await launch(tester, size);
      for (final (page, label) in [(SfPage.devices, '复制指纹'), (SfPage.about, '检查更新')]) {
        state.go(page);
        await settle(tester);
        final banner = tester.getRect(find.byType(BannerRow));
        final button = tester.getRect(find.ancestor(of: find.text(label), matching: find.byType(PillButton)));
        if (wide) {
          // 按钮贴着横幅右边，和左边的图标一样留出卡片内边距。
          expect(button.right, moreOrLessEquals(banner.right), reason: label);
          expect(button.center.dy, moreOrLessEquals(banner.center.dy, epsilon: 1), reason: label);
        } else {
          expect(button.left, moreOrLessEquals(banner.left), reason: label);
          expect(button.bottom, moreOrLessEquals(banner.bottom), reason: label);
        }
      }
    });
  }

  for (final (name, size) in [('点按钮收起', const Size(1160, 780)), ('窗口太窄自动收起', const Size(600, 900))]) {
    testWidgets('侧栏$name后图标和状态圆点都居中', (tester) async {
      final state = await launch(tester, size);
      if (!state.sidebarCollapsed && size.width >= 900) {
        await tester.tap(find.byTooltip('收起侧栏'));
        await settle(tester);
      }
      final side = tester.getRect(find.byType(Sidebar));
      expect(side.width, collapsedWidth);
      // 内容区左右各留 14，右边还有 1 点分隔线。
      final center = side.left + 14 + (collapsedWidth - 29) / 2;
      final marks = [
        ...find.descendant(of: find.byType(Sidebar), matching: find.byType(GlyphIcon)).evaluate(),
        ...find.descendant(of: find.byType(Sidebar), matching: find.byType(StatusDot)).evaluate(),
      ];
      expect(marks.length, greaterThanOrEqualTo(6));
      for (final e in marks) {
        expect(tester.getCenter(find.byElementPredicate((x) => x == e)).dx, moreOrLessEquals(center), reason: '$e');
      }
    });
  }

  testWidgets('手机大小时用底部标签栏切换页面', (tester) async {
    final state = await launch(tester, const Size(375, 667));
    expect(find.byType(Sidebar), findsNothing);
    for (final (label, page) in [('设备', SfPage.devices), ('日志', SfPage.log), ('仪表盘', SfPage.overview)]) {
      await tester.tap(find.descendant(of: find.byType(BottomNav), matching: find.text(label)));
      await settle(tester);
      expect(state.page, page);
    }

    // 窗口拉宽后换回侧栏，当前页面不变。
    tester.view.physicalSize = const Size(1160, 780);
    await settle(tester);
    expect(find.byType(BottomNav), findsNothing);
    expect(find.byType(Sidebar), findsOneWidget);
    expect(state.page, SfPage.overview);
  });

  testWidgets('缺少权限时显示提示条，按钮请后台打开系统设置', (tester) async {
    final backend = DemoBackend(
      animate: false,
      problem: const Problem(kind: ProblemKind.permission, message: '需要在“隐私与安全性”中允许 ScreenFerry 控制电脑'),
    );
    await launch(tester, const Size(1160, 780), backend: backend);
    expect(find.text('需要键鼠控制权限'), findsOneWidget);
    expect(find.text('暂时无法共享'), findsOneWidget, reason: '主卡也说明了状态');

    await tester.tap(find.text('打开系统设置'));
    await settle(tester);
    expect(backend.sent.whereType<OpenPermissionSettingsRequest>(), hasLength(1));

    backend.problem = null;
    await settle(tester);
    expect(find.text('需要键鼠控制权限'), findsNothing);
  });

  testWidgets('对方发起配对时弹出配对码，后台说结束就自动关闭', (tester) async {
    final backend = DemoBackend(animate: false);
    await launch(tester, const Size(1160, 780), backend: backend);

    final id = backend.requestPairing('会议室 PC');
    await settle(tester);
    expect(find.text('与“会议室 PC”配对'), findsOneWidget);
    // 只在对话框里找：首页的统计数字随时间变化，也可能恰好是 4。
    final dialog = find.byWidgetPredicate((w) => w.runtimeType.toString() == '_PairingDialog');
    expect(
      find.descendant(of: dialog, matching: find.text('4')),
      findsOneWidget,
      reason: '配对码逐位显示',
    );

    backend.emit(PairingClosedNotice(id));
    await settle(tester);
    expect(find.text('与“会议室 PC”配对'), findsNothing);
  });

  testWidgets('拒绝配对会告诉后台并关闭对话框', (tester) async {
    final backend = DemoBackend(animate: false);
    await launch(tester, const Size(1160, 780), backend: backend);
    final id = backend.requestPairing('会议室 PC');
    await settle(tester);

    await tester.tap(find.text('拒绝'));
    await settle(tester);
    final reply = backend.sent.whereType<PairReplyRequest>().single;
    expect((reply.id, reply.accept), (id, false));
    expect(find.text('与“会议室 PC”配对'), findsNothing);
  });

  testWidgets('后台没有运行时显示占位页，侧栏仍在', (tester) async {
    final state = await launch(tester, const Size(1160, 780), backend: DemoBackend(animate: false, startOnline: false));
    expect(find.text('后台没有运行'), findsOneWidget);
    expect(find.text('请先启动 ScreenFerry，连上后会自动显示。'), findsOneWidget);
    expect(navItem('设备'), findsOneWidget);
    expect(find.text('往返延迟'), findsNothing);
    expect(state.online, isFalse);
  });

  testWidgets('与后台断开后保留数据、提示重连，并禁用会发命令的控件', (tester) async {
    final backend = DemoBackend(animate: false);
    final state = await launch(tester, const Size(1160, 780), backend: backend);
    expect(find.text('往返延迟'), findsOneWidget);

    backend.goOffline();
    await settle(tester);
    expect(find.text('与后台的连接已断开，正在重连…'), findsOneWidget);
    expect(find.text('往返延迟'), findsOneWidget, reason: '保留最后的数据');

    final sentBefore = backend.sent.length;
    await tester.tap(find.text('剪贴板同步'));
    await tester.tap(find.byTooltip('暂停共享'));
    await settle(tester);
    expect(backend.sent, hasLength(sentBefore), reason: '断开时不发命令');
    expect(state.settings.clipboard, isTrue);

    backend.goOnline();
    await settle(tester);
    expect(find.text('与后台的连接已断开，正在重连…'), findsNothing);
  });

  testWidgets('设置开关立即生效，并把改动发给后台', (tester) async {
    final backend = DemoBackend(animate: false);
    final state = await launch(tester, const Size(1160, 780), backend: backend);
    state.go(SfPage.settings);
    await settle(tester);

    // 同名的有标题文字和开关两处，开关在后。
    await tester.tap(find.bySemanticsLabel('同步剪贴板').last);
    await settle(tester);
    expect(state.settings.clipboard, isFalse);
    final patch = backend.sent.whereType<UpdateSettingsRequest>().last.settings;
    expect(patch.toJson(), {'clipboard': false});
  });
  for (final (name, size) in [
    ('默认窗口', const Size(1160, 780)),
    ('窄屏', const Size(600, 900)),
    ('手机大小', const Size(375, 667)),
  ]) {
    testWidgets('$name下布局页把两台摆在本机右侧、一台摆在左侧，都在画布里且互不重叠', (tester) async {
      final state = await launch(tester, size);
      final (office, study, notebook) = await openLayout(tester, state);
      for (final name in ['办公室 PC', '书房 PC', '家里的笔记本']) {
        expect(find.text(name), findsOneWidget);
      }
      final local = screenRect(tester, 'local');
      final (a, b, n) = (screenRect(tester, office), screenRect(tester, study), screenRect(tester, notebook));
      expect(a.left, greaterThan(local.right));
      expect(b.left, greaterThan(local.right));
      expect(a.bottom, lessThan(b.top), reason: '同一侧按先后从上往下排');
      expect(n.right, lessThan(local.left));

      final stage = tester.getRect(
        find.ancestor(of: find.byKey(const ValueKey('screen-local')), matching: find.byType(Stack)).first,
      );
      final rects = [local, a, b, n];
      for (var i = 0; i < rects.length; i++) {
        expect(stage.contains(rects[i].topLeft) && stage.contains(rects[i].bottomRight), isTrue, reason: '第 $i 个框');
        for (var j = i + 1; j < rects.length; j++) {
          expect(rects[i].overlaps(rects[j]), isFalse, reason: '第 $i 和第 $j 个框重叠');
        }
      }
      expect(find.text('本机左侧 1 台、右侧 2 台。同一侧已连接的几台按先后平分那条边，光标从哪一段穿过去就到哪台'), findsOneWidget);
    });
  }

  testWidgets('把右侧的一台拖到左侧最上面，吸附过去，发出的 layout 按新的先后列出全部设备', (tester) async {
    final backend = DemoBackend(animate: false);
    final state = await launch(tester, const Size(1160, 780), backend: backend);
    final (office, study, notebook) = await openLayout(tester, state);
    final local = screenRect(tester, 'local');
    final from = screenRect(tester, study).center;
    // 左侧放两台时，第一台正对本机左边的上半段。
    final to = Offset(screenRect(tester, notebook).center.dx, local.top + local.height / 4);

    final gesture = await tester.startGesture(from);
    await gesture.moveBy(const Offset(-30, 0));
    await tester.pump();
    await gesture.moveTo(to);
    await tester.pump();
    expect(find.text('松手后放到本机左侧，从上往下第 1 台'), findsOneWidget, reason: '拖动时说明会落到哪里');
    await gesture.up();
    await settle(tester);

    final layout = backend.sent.whereType<UpdateSettingsRequest>().last.settings.layout;
    expect(layout, [
      Placement(study, ScreenSide.left),
      Placement(notebook, ScreenSide.left),
      Placement(office, ScreenSide.right),
    ]);
    final moved = screenRect(tester, study);
    expect(moved.right, lessThan(screenRect(tester, 'local').left));
    expect(moved.bottom, lessThan(screenRect(tester, notebook).top));
    expect(state.placeOf(study), (side: ScreenSide.left, index: 0));
  });

  testWidgets('在同一侧拖动可以调整先后', (tester) async {
    final backend = DemoBackend(animate: false);
    final state = await launch(tester, const Size(1160, 780), backend: backend);
    final (office, study, notebook) = await openLayout(tester, state);
    final from = screenRect(tester, study).center;
    await tester.dragFrom(from, screenRect(tester, office).center - from);
    await settle(tester);

    expect(backend.sent.whereType<UpdateSettingsRequest>().last.settings.layout, [
      Placement(notebook, ScreenSide.left),
      Placement(study, ScreenSide.right),
      Placement(office, ScreenSide.right),
    ]);
    expect(screenRect(tester, study).bottom, lessThan(screenRect(tester, office).top));
  });

  testWidgets('拖回原处不发命令；与后台断开时拖不动', (tester) async {
    final backend = DemoBackend(animate: false);
    final state = await launch(tester, const Size(1160, 780), backend: backend);
    final (_, study, _) = await openLayout(tester, state);
    await tester.dragFrom(screenRect(tester, study).center, const Offset(4, 3));
    await settle(tester);
    expect(backend.sent.whereType<UpdateSettingsRequest>(), isEmpty);

    backend.goOffline();
    await settle(tester);
    final before = screenRect(tester, study);
    await tester.dragFrom(before.center, const Offset(-600, 0));
    await settle(tester);
    expect(backend.sent.whereType<UpdateSettingsRequest>(), isEmpty);
    expect(screenRect(tester, study), before);
  });

  testWidgets('光标所在的设备有标识，状态区分已连接和未连接', (tester) async {
    final backend = DemoBackend(animate: false);
    final state = await launch(tester, const Size(1160, 780), backend: backend);
    final (office, _, _) = await openLayout(tester, state);
    expect(find.text('光标在这里'), findsNothing);
    expect(find.text('已连接'), findsNWidgets(2));
    expect(find.text('未连接'), findsOneWidget);

    backend.cursor = office;
    await settle(tester);
    expect(find.text('光标在这里'), findsOneWidget);
    expect(find.bySemanticsLabel(RegExp('^办公室 PC.*光标在这台上')), findsOneWidget);
    expect(state.cursorPeer?.name, '办公室 PC');
  });

  testWidgets('只有一台设备时说明它在哪一侧，没有设备时提示去配对', (tester) async {
    final state = await launch(tester, const Size(1160, 780));
    final (_, study, notebook) = await openLayout(tester, state);
    for (final fp in [study, notebook]) {
      state.forget(state.peers.firstWhere((p) => p.fingerprint == fp));
    }
    await settle(tester);
    expect(find.text('办公室 PC 在本机右侧，光标从相接的那条边穿过去'), findsOneWidget);
  });

  testWidgets('还没有配对的设备时布局页只显示本机', (tester) async {
    final state = await launch(tester, const Size(1160, 780), backend: DemoBackend(animate: false, paired: false));
    state.go(SfPage.layout);
    await settle(tester);
    expect(find.byKey(const ValueKey('screen-local')), findsOneWidget);
    expect(find.textContaining('还没有配对的设备'), findsOneWidget);
    bool isDevice(Widget w) => switch (w.key) {
      ValueKey<String>(:final value) => value.startsWith('screen-') && value != 'screen-local',
      _ => false,
    };
    expect(find.byWidgetPredicate(isDevice), findsNothing, reason: '没有设备框');

    await tester.tap(find.text('去配对'));
    await settle(tester);
    expect(state.page, SfPage.devices);
  });
}
