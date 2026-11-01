// 布局的先后规则与移动后写出的 layout。

import 'package:flutter_test/flutter_test.dart';
import 'package:screenferry_settings/state/app_state.dart';

PeerInfo peer(String name) =>
    PeerInfo(name: name, fingerprint: name.padRight(64, '0'), short: '', online: true, lastSeen: null);

DaemonSettings settings(ScreenSide side, [List<Placement> layout = const []]) => DaemonSettings(
  name: '',
  side: side,
  layout: layout,
  announce: true,
  clipboard: true,
  server: '',
  autostart: false,
  theme: UiTheme.system,
  accent: 0,
);

List<String> names(Arrangement a, ScreenSide side) => [for (final p in a.on(side)) p.name];

void main() {
  final a = peer('a'), b = peer('b'), c = peer('c'), d = peer('d');

  test('没有单独安排时都放在默认的一侧，按已配对的顺序', () {
    final arr = Arrangement.of([a, b, c], settings(ScreenSide.left));
    expect(names(arr, ScreenSide.left), ['a', 'b', 'c']);
    expect(names(arr, ScreenSide.right), isEmpty);
    expect(arr.placeOf(c.fingerprint), (side: ScreenSide.left, index: 2));
  });

  test('先按 layout 的先后，没列出的排在默认一侧的后面', () {
    final arr = Arrangement.of(
      [a, b, c, d],
      settings(ScreenSide.right, [
        Placement(c.fingerprint, ScreenSide.right),
        Placement(b.fingerprint, ScreenSide.left),
        Placement('gone'.padRight(64, '0'), ScreenSide.top),
      ]),
    );
    expect(names(arr, ScreenSide.right), ['c', 'a', 'd']);
    expect(names(arr, ScreenSide.left), ['b']);
    expect(names(arr, ScreenSide.top), isEmpty, reason: '已经不在的设备忽略');
    expect(arr.usedSides, [ScreenSide.left, ScreenSide.right]);
  });

  test('移到另一侧、同一侧换先后，写出的 layout 列出全部设备', () {
    final arr = Arrangement.of([a, b, c], settings(ScreenSide.right, [Placement(c.fingerprint, ScreenSide.left)]));
    // 右侧 a、b，左侧 c。把 b 移到左侧最前面。
    final moved = arr.moved(b.fingerprint, ScreenSide.left, 0);
    expect(names(moved, ScreenSide.left), ['b', 'c']);
    expect(names(moved, ScreenSide.right), ['a']);
    expect(moved.toLayout(), [
      Placement(b.fingerprint, ScreenSide.left),
      Placement(c.fingerprint, ScreenSide.left),
      Placement(a.fingerprint, ScreenSide.right),
    ]);

    // 同一侧换先后：位置按去掉它之后算，超出范围的放到最后。
    final swapped = arr.moved(a.fingerprint, ScreenSide.right, 9);
    expect(names(swapped, ScreenSide.right), ['b', 'a']);
    expect(arr.moved(a.fingerprint, ScreenSide.right, 0).sameAs(arr), isTrue);
    expect(arr.moved('x', ScreenSide.top, 0).sameAs(arr), isTrue, reason: '不认识的设备不动');
  });
}
