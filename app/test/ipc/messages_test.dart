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

const pc = '3f9a12c477e0b1d2aa55aa55aa55aa55aa55aa55aa55aa55aa55aa55aa55aa55';
const mac = '7ab05cb238fbd60311223344556677889900aabbccddeeff0011223344556677';

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
      ShowDeviceNotice,
      DialedNotice,
      QuitNotice,
      RunLogNotice,
      DiagnosticsExportedNotice,
      DiagnosticsSummaryNotice,
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
    expect(s.local.port, 24810);
    expect(s.local.platform, 'macos');
    expect(s.local.naturalScrolling, isTrue);
    expect(s.local.addresses, ['192.168.2.12', '10.8.0.3']);
    expect(s.local.invitePort, isNull, reason: '控制端不等邀请');
    expect(s.local.system, 'macOS 15.6 · arm64');
    expect(s.local.verboseSince, DateTime.fromMillisecondsSinceEpoch(1793480000000));
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
    expect(s.clipboard!.rich, isTrue);
    expect(s.clipboard!.image!.width, 1920);
    expect(s.clipboard!.image!.height, 1080);
    expect(s.clipboard!.image!.path, endsWith('blobs/ab/cdef'));
    expect(s.settings.clipboardLimitMb, 32);
    expect(s.peers.single.online, isTrue);
    expect(s.peers.single.lastSeen, isNull);
    expect(s.peers.single.side, ScreenSide.left);
    expect(s.peers.single.cursor, isTrue);
    expect(s.peers.single.alias, '书房的电脑');
    expect(s.peers.single.addr, '192.168.2.18');
    expect(s.peers.single.platform, 'windows');
    expect(s.peers.single.displayName, '书房的电脑', reason: '起过名字就显示起的名字');
    expect(s.settings.mouseDpi, 1600);
    final d = s.settings.devices.single;
    expect(d.fingerprint, pc);
    expect(d.alias, '书房的电脑');
    expect(d.pointerDpi, 2000);
    expect(d.reverseVertical && d.reverseHorizontal, isTrue);
    expect(d.scrollSpeed, ScrollSpeed.fast);
    expect(d.modifiers, ModifierPreset.custom);
    expect(d.textShortcuts, isTrue);
    expect(d.edgeDelayMs, 200);
    expect(d.clipboard || d.files, isFalse);
    expect(d.customModifiers.capsLock, ModifierTarget.ctrl);
    expect(d.customModifiers.alt, ModifierTarget.none);
    expect(d.customModifiers.gui, ModifierTarget.gui);
    expect(d.keys, const [KeyMapping(0x00070068, 0x00070046), KeyMapping(0x00070039, 0)]);
    expect(s.settings.device(pc), d);
    expect(s.settings.device(mac), const DeviceSettings(fingerprint: mac), reason: '没单独设置过的用缺省设置');
    expect(s.found.single.paired, isFalse);
    expect(s.settings.side, ScreenSide.right);
    expect(s.settings.layout, [const Placement(pc, ScreenSide.left)]);
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
    expect(decoded.whereType<ShowDeviceNotice>().single.fingerprint, pc);
    final dialed = decoded.whereType<DialedNotice>().toList();
    expect(dialed.first.address, '192.168.2.12');
    expect(dialed.first.invite, isFalse);
    expect(dialed.first.error, isNull);
    expect(dialed.last.invite, isTrue);
    expect(dialed.last.error, DialError.notScreenFerry);
    final runLog = decoded.whereType<RunLogNotice>().single;
    expect(runLog.fresh, isTrue);
    expect(runLog.next, 204800);
    expect(runLog.linesTotal, 18241);
    expect(runLog.lines.first.at, '06:12:04');
    expect(runLog.lines.last.level, RunLogLevel.warn);
    expect(decoded.whereType<DiagnosticsExportedNotice>().single.bytes, 421888);
    expect(decoded.whereType<DiagnosticsSummaryNotice>().single.text, startsWith('ScreenFerry 诊断摘要'));
  });

  test('Dart 编码的命令与样例一致', () {
    final ours = <Request>[
      HelloRequest('ab' * 32),
      const SetSharingRequest(false),
      const UpdateSettingsRequest(
        SettingsPatch(
          side: ScreenSide.left,
          autostart: true,
          theme: UiTheme.light,
          clipboardLimitMb: 16,
          mouseDpi: 800,
        ),
      ),
      const UpdateSettingsRequest(
        SettingsPatch(layout: [Placement(pc, ScreenSide.left), Placement(mac, ScreenSide.right)]),
      ),
      const UpdateSettingsRequest(SettingsPatch(role: Role.client)),
      const SetDeviceRequest(
        DeviceSettings(
          fingerprint: pc,
          alias: '书房的电脑',
          pointerDpi: 2000,
          reverseVertical: true,
          reverseHorizontal: true,
          scrollSpeed: ScrollSpeed.fast,
          modifiers: ModifierPreset.custom,
          textShortcuts: true,
          edgeDelayMs: 200,
          clipboard: false,
          files: false,
          customModifiers: ModifierMap(capsLock: ModifierTarget.ctrl, alt: ModifierTarget.none),
          keys: [KeyMapping(0x00070068, 0x00070046), KeyMapping(0x00070039, 0)],
        ),
      ),
      const SetDeviceRequest(DeviceSettings(fingerprint: mac)),
      const ForgetRequest(pc),
      const PairReplyRequest(3, accept: true),
      const ConnectRequest(mac),
      const ConnectAddressRequest('192.168.2.12'),
      const InviteRequest('192.168.2.18:24811'),
      const OpenPermissionSettingsRequest(),
      const ClearLogRequest(),
      const OpenSettingsRequest(),
      const OpenPeerSettingsRequest(pc),
      const SendFilesRequest(['/Users/me/Desktop/证书.p12', '/Users/me/Desktop/设计稿']),
      const SendFilesRequest(['/Users/me/Desktop/证书.p12'], to: pc),
      const QuitRequest(),
      const ReadRunLogRequest(),
      const ReadRunLogRequest(since: 204800),
      const ExportDiagnosticsRequest(),
      const CopyDiagnosticsRequest(),
      const RevealDiagnosticsRequest(),
      const OpenLogsFolderRequest(),
      const SetVerboseRequest(true),
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

  test('旧版后台的数据缺少多设备字段时取缺省值', () {
    final peer = PeerInfo.fromJson({'name': '办公室 PC', 'fingerprint': pc, 'short': '', 'online': true});
    expect(peer.side, isNull);
    expect(peer.cursor, isFalse);
    expect(PeerInfo.fromJson({'side': 'diagonal'}).side, isNull, reason: '不认识的取值当作不知道');
    final settings = DaemonSettings.fromJson({'name': '', 'side': 'left'});
    expect(settings.side, ScreenSide.left);
    expect(settings.layout, isEmpty);
  });

  test('设置补丁只发出改了的字段', () {
    expect(const SettingsPatch(clipboard: false).toJson(), {'clipboard': false});
    expect(const SettingsPatch().isEmpty, isTrue);
    expect(const SettingsPatch(layout: []).toJson(), {'layout': <Object?>[]}, reason: '空的 layout 也要发出去');
  });

  test('后台确认了同样的布局后，补丁里不再带 layout', () {
    const layout = [Placement(pc, ScreenSide.left), Placement(mac, ScreenSide.right)];
    final confirmed = DaemonSettings.fromJson({
      'side': 'right',
      'layout': [for (final p in layout) p.toJson()],
    });
    expect(const SettingsPatch(layout: layout).without(confirmed).isEmpty, isTrue);
    final other = DaemonSettings.fromJson({'side': 'right'});
    expect(const SettingsPatch(layout: layout).without(other).layout, layout);
    expect(other.apply(const SettingsPatch(layout: layout)).layout, layout);
  });

  test('设备设置：缺省值、恢复默认、修饰键方案', () {
    const d = DeviceSettings(fingerprint: pc);
    expect(d.isDefault, isTrue);
    expect(d.modifierMap(differ: true), ModifierMap.swapGuiCtrl, reason: '本机习惯：两种键盘不同时互换');
    expect(d.modifierMap(differ: false), ModifierMap.plain);
    final changed = d.copyWith(alias: '书房', reverseVertical: true, keys: const [KeyMapping(1, 2)]);
    expect(changed.isDefault, isFalse);
    expect(
      changed.reset(),
      const DeviceSettings(fingerprint: pc, alias: '书房'),
      reason: '恢复默认保留名字',
    );
    expect(ModifierMap.plain.replaced(ModifierKey.capsLock, ModifierTarget.ctrl).toJson(), {
      'gui': 'gui',
      'ctrl': 'ctrl',
      'alt': 'alt',
      'caps_lock': 'ctrl',
    });
  });

  test('先按改了的设备设置显示，改回缺省时去掉这一条', () {
    final s = DaemonSettings.fromJson({'side': 'right'});
    final mine = s.withDevice(const DeviceSettings(fingerprint: pc, reverseVertical: true));
    expect(mine.device(pc).reverseVertical, isTrue);
    expect(mine.devices, hasLength(1));
    expect(mine.withDevice(const DeviceSettings(fingerprint: pc)).devices, isEmpty);
  });

  test('旧版后台没有设备设置和鼠标 DPI 时取缺省值', () {
    final settings = DaemonSettings.fromJson({'name': '', 'side': 'left'});
    expect(settings.mouseDpi, 1600);
    expect(settings.devices, isEmpty);
    final peer = PeerInfo.fromJson({'name': '办公室 PC', 'fingerprint': pc});
    expect(peer.alias, isEmpty);
    expect(peer.displayName, '办公室 PC');
    expect(Local.fromJson({}).naturalScrolling, isNull);
  });
}
