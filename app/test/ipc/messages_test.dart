// 与 Rust 共用的样例（test/fixtures/ipc_samples.json，由 ferry-ipc 的测试生成）：
// 每条通知都能解码，每条命令用 Dart 构造后编码，与样例逐字段相等。

import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:screenferry_settings/ipc/messages.dart';

/// 不看键的顺序，逐层比较两个 JSON 值。
bool sameJson(Object? a, Object? b) {
  if (a is Map && b is Map) {
    return a.length == b.length && a.keys.every((k) => b.containsKey(k) && sameJson(a[k], b[k]));
  }
  if (a is List && b is List) {
    return a.length == b.length && [for (var i = 0; i < a.length; i++) sameJson(a[i], b[i])].every((x) => x);
  }
  if (a is num && b is num) return a == b;
  return a == b;
}

void main() {
  final samples = jsonDecode(File('test/fixtures/ipc_samples.json').readAsStringSync()) as Map<String, Object?>;
  final notices = (samples['notices']! as List).cast<Map<String, Object?>>();
  final requests = (samples['requests']! as List).cast<Map<String, Object?>>();

  test('样例与 Dart 使用同一个协议版本', () {
    expect(samples['version'], ipcVersion);
  });

  test('每条通知都能解码', () {
    final decoded = [for (final n in notices) Notice.fromJson(n)];
    expect(decoded, everyElement(isNotNull));
    expect(decoded.map((n) => n.runtimeType).toSet(), {
      WelcomeNotice,
      StateNotice,
      HistoryNotice,
      LogNotice,
      PairingNotice,
      PairingClosedNotice,
      ToastNotice,
    });
  });

  test('状态快照的字段都读对了', () {
    final states = [
      for (final n in notices.map(Notice.fromJson))
        if (n is StateNotice) n.state,
    ];
    final s = states.first;
    expect(s.local.name, 'Mac Studio');
    expect(s.local.role, Role.server);
    expect(s.local.short, '7AB0-5CB2-38FB-D603');
    expect(s.local.version, '0.1.0');
    expect(s.sharing, isTrue);
    expect(s.problem, isNull);
    expect(s.link.status, LinkStatus.connected);
    expect(s.link.peer!.name, '办公室 PC');
    expect(s.link.peer!.addr, '192.168.2.18');
    expect(s.link.since, DateTime.fromMillisecondsSinceEpoch(1793480000000));
    expect(s.link.cursorOnPeer, isTrue);
    // Rust 侧是 f32，JSON 里会带上浮点误差。
    expect(s.link.rttMs.map((v) => v.toStringAsFixed(2)), ['0.31', '0.28', '0.50']);
    expect(s.clipboard!.toPeer, isTrue);
    expect(s.clipboard!.chars, 31);
    expect(s.clipboard!.preview, '周三例会纪要');
    expect(s.peers.single.online, isTrue);
    expect(s.peers.single.lastSeen, isNull);
    expect(s.found.single.paired, isFalse);
    expect(s.settings.side, ScreenSide.right);
    expect(s.settings.theme, UiTheme.dark);
    expect(s.settings.accent, 2);
    expect(s.activity.single.switches, 12);
    expect(s.activity.single.hour, DateTime.fromMillisecondsSinceEpoch(1793476800 * 1000));

    final stopped = states[1];
    expect(stopped.sharing, isFalse);
    expect(stopped.problem!.kind, ProblemKind.permission);
    expect(stopped.link.status, LinkStatus.stopped);
    expect(stopped.link.peer, isNull);
    expect(stopped.link.rttMs, isEmpty);
    expect(stopped.clipboard, isNull);
  });

  test('日志、配对与提示', () {
    final decoded = notices.map(Notice.fromJson).toList();
    final history = decoded.whereType<HistoryNotice>().single;
    expect(history.entries.single.kind, LogKind.cursor);
    expect(history.entries.single.icon, LogIcon.cursor);
    expect(decoded.whereType<LogNotice>().single.entry.text, '光标切到 办公室 PC');
    final prompt = decoded.whereType<PairingNotice>().single.prompt;
    expect(prompt.id, 3);
    expect(prompt.peerName, '会议室 PC');
    expect(prompt.digits, '482913');
    expect(decoded.whereType<PairingClosedNotice>().single.id, 3);
    expect(decoded.whereType<ToastNotice>().single.text, '已暂停共享');
  });

  test('Dart 编码的命令与样例一致', () {
    const pc = '3f9a12c477e0b1d2aa55aa55aa55aa55aa55aa55aa55aa55aa55aa55aa55aa55';
    const mac = '7ab05cb238fbd60311223344556677889900aabbccddeeff0011223344556677';
    final ours = <Request>[
      HelloRequest('ab' * 32),
      const SetSharingRequest(false),
      const UpdateSettingsRequest(SettingsPatch(side: ScreenSide.left, autostart: true, theme: UiTheme.light)),
      const ForgetRequest(pc),
      const PairReplyRequest(3, accept: true),
      const ConnectRequest(mac),
      const OpenPermissionSettingsRequest(),
      const ClearLogRequest(),
      const OpenSettingsRequest(),
    ];
    expect(ours, hasLength(requests.length));
    for (var i = 0; i < ours.length; i++) {
      final json = ours[i].toJson();
      expect(
        sameJson(json, requests[i]),
        isTrue,
        reason: '第 $i 条：Dart ${jsonEncode(json)}，样例 ${jsonEncode(requests[i])}',
      );
    }
  });

  test('不认识的通知和取值不会出错', () {
    expect(Notice.fromJson({'type': 'from_the_future', 'x': 1}), isNull);
    final link = Link.fromJson({'status': 'teleporting', 'later': true});
    expect(link.status, LinkStatus.starting);
    expect(Problem.fromJson({'kind': 'port_in_use', 'message': ''}).kind, ProblemKind.portInUse);
  });

  test('设置补丁只发出改了的字段', () {
    expect(const SettingsPatch(clipboard: false).toJson(), {'clipboard': false});
    expect(const SettingsPatch().isEmpty, isTrue);
  });
}
