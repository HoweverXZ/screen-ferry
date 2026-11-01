// 服务端四周的设备布局：每台已配对设备在本机哪一侧、在那一侧排第几。
// 规则与后台一致：先按 settings.layout 里的先后；没有列出的放在 settings.side 一侧、
// 排在已安排的后面，按已配对设备列表的顺序。

import '../ipc/messages.dart';

/// 一台设备的位置：在本机哪一侧、在那一侧排第几（从 0 开始；左右两侧从上往下，上下两侧从左往右）。
typedef PeerPlace = ({ScreenSide side, int index});

class Arrangement {
  const Arrangement._(this._sides);

  /// 按设置算出每台设备的位置。layout 里有、但已经不在 [peers] 里的设备忽略。
  factory Arrangement.of(List<PeerInfo> peers, DaemonSettings settings) {
    final byFingerprint = {for (final p in peers) p.fingerprint: p};
    final sides = {for (final s in ScreenSide.values) s: <PeerInfo>[]};
    final placed = <String>{};
    for (final p in settings.layout) {
      final peer = byFingerprint[p.fingerprint];
      if (peer == null || !placed.add(p.fingerprint)) continue;
      sides[p.side]!.add(peer);
    }
    for (final peer in peers) {
      if (placed.add(peer.fingerprint)) sides[settings.side]!.add(peer);
    }
    return Arrangement._(sides);
  }

  final Map<ScreenSide, List<PeerInfo>> _sides;

  /// 这一侧的设备，按先后排列。
  List<PeerInfo> on(ScreenSide side) => List.unmodifiable(_sides[side]!);

  /// 所有设备，按 [ScreenSide.values] 的顺序逐侧列出。
  Iterable<PeerInfo> get all => ScreenSide.values.expand((s) => _sides[s]!);

  int get length => _sides.values.fold(0, (n, l) => n + l.length);

  bool get isEmpty => length == 0;

  /// 有设备的那几侧。
  List<ScreenSide> get usedSides => [
    for (final s in ScreenSide.values)
      if (_sides[s]!.isNotEmpty) s,
  ];

  PeerPlace? placeOf(String fingerprint) {
    for (final s in ScreenSide.values) {
      final i = _sides[s]!.indexWhere((p) => p.fingerprint == fingerprint);
      if (i >= 0) return (side: s, index: i);
    }
    return null;
  }

  /// 去掉一台设备后的布局。
  Arrangement without(String fingerprint) => Arrangement._({
    for (final s in ScreenSide.values) s: [..._sides[s]!.where((p) => p.fingerprint != fingerprint)],
  });

  /// 把一台设备移到 [side] 一侧的第 [index] 位（按去掉它之后的那一侧算，超出范围的放到最后）。
  /// 设备不在布局里时原样返回。
  Arrangement moved(String fingerprint, ScreenSide side, int index) {
    final peer = all.where((p) => p.fingerprint == fingerprint).firstOrNull;
    if (peer == null) return this;
    final rest = without(fingerprint);
    final list = rest._sides[side]!;
    list.insert(index.clamp(0, list.length), peer);
    return rest;
  }

  /// 写进设置的完整 layout：每台设备都单独安排，之后不再受 settings.side 影响。
  List<Placement> toLayout() => [
    for (final s in ScreenSide.values)
      for (final p in _sides[s]!) Placement(p.fingerprint, s),
  ];

  /// 两份布局里每台设备的位置都相同。
  bool sameAs(Arrangement other) => ScreenSide.values.every((s) {
    final a = _sides[s]!, b = other._sides[s]!;
    if (a.length != b.length) return false;
    for (var i = 0; i < a.length; i++) {
      if (a[i].fingerprint != b[i].fingerprint) return false;
    }
    return true;
  });
}
