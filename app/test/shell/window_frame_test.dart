import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:screenferry_settings/app.dart';
import 'package:screenferry_settings/shell/window_frame.dart';
import 'package:screenferry_settings/state/app_state.dart';

import '../support.dart';

const _channel = MethodChannel('screenferry/window');

Future<void> settle(WidgetTester tester) async {
  for (var i = 0; i < 6; i++) {
    await tester.pump(const Duration(milliseconds: 150));
  }
}

/// 启动界面；`frame` 为真时按 Windows 的样子自己画窗口按钮。记下发给原生一侧的调用。
Future<List<String>> launch(WidgetTester tester, {required bool frame}) async {
  tester.view.physicalSize = const Size(1160, 780);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  final calls = <String>[];
  tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(_channel, (call) async {
    calls.add(call.method);
    return call.method == 'isMaximized' ? false : null;
  });
  addTearDown(() => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(_channel, null));
  useLocale(tester);
  final state = AppState(backend: DemoBackend(animate: false));
  addTearDown(state.dispose);
  await tester.pumpWidget(ScreenFerryApp(state: state, windowFrame: frame));
  state.start();
  await settle(tester);
  return calls;
}

/// 模拟原生一侧通知最大化状态。
Future<void> notifyMaximized(WidgetTester tester, bool maximized) async {
  final message = const StandardMethodCodec().encodeMethodCall(MethodCall('maximized', maximized));
  await tester.binding.defaultBinaryMessenger.handlePlatformMessage(_channel.name, message, (_) {});
  await settle(tester);
}

void main() {
  testWidgets('Windows 上右上角有三个窗口按钮，点了交给原生窗口执行', (tester) async {
    final calls = await launch(tester, frame: true);
    for (final label in ['最小化', '最大化', '关闭']) {
      expect(find.byTooltip(label), findsOneWidget, reason: label);
    }
    // 按钮在右上角，且都在顶部 32 点那一行里。
    final close = tester.getRect(find.byTooltip('关闭'));
    expect(close.right, greaterThan(1100));
    expect(close.bottom, lessThanOrEqualTo(windowBarHeight));

    await tester.tap(find.byTooltip('最小化'));
    await tester.tap(find.byTooltip('最大化'));
    await tester.tap(find.byTooltip('关闭'));
    await settle(tester);
    expect(calls, containsAllInOrder(['minimize', 'toggleMaximize', 'close']));
  });

  testWidgets('最大化后按钮变成“还原”', (tester) async {
    await launch(tester, frame: true);
    await notifyMaximized(tester, true);
    expect(find.byTooltip('还原'), findsOneWidget);
    await notifyMaximized(tester, false);
    expect(find.byTooltip('最大化'), findsOneWidget);
  });

  testWidgets('拖动顶部那一行会请原生窗口移动', (tester) async {
    final calls = await launch(tester, frame: true);
    await tester.dragFrom(const Offset(500, 16), const Offset(80, 40));
    await settle(tester);
    expect(calls, contains('startDrag'));
  });

  testWidgets('macOS 用系统的红绿灯，不画窗口按钮', (tester) async {
    await launch(tester, frame: false);
    expect(find.byType(WindowFrame), findsNothing);
    expect(find.byTooltip('关闭'), findsNothing);
  });

  testWidgets('没有原生一侧时也不出错', (tester) async {
    tester.view.physicalSize = const Size(1160, 780);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    useLocale(tester);
    final state = AppState(backend: DemoBackend(animate: false));
    addTearDown(state.dispose);
    await tester.pumpWidget(ScreenFerryApp(state: state, windowFrame: true));
    await settle(tester);
    await tester.tap(find.byTooltip('关闭'));
    await settle(tester);
    expect(tester.takeException(), isNull);
  });
}
