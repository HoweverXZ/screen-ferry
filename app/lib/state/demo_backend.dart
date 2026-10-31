// 演示用的假后台：不连真实的后台，按协议推送模拟的状态、回应命令。
// 供 widget 测试使用，也方便单独看界面设计（例如 `flutter run --dart-define=SF_DEMO=true`）。

import 'dart:async';
import 'dart:math';

import 'package:flutter/foundation.dart';

import '../ipc/messages.dart';
import 'backend.dart';

class DemoBackend implements Backend {
  DemoBackend({this.role = Role.server, this.animate = true, this.startOnline = true, this._problem}) {
    final now = DateTime.now();
    _since = now.subtract(const Duration(hours: 1, minutes: 12, seconds: 14));
    _clip = ClipboardSync(
      toPeer: true,
      peer: _peerName,
      chars: 31,
      bytes: 93,
      preview: '周三例会纪要：确认屏幕布局、配对流程，以及剪贴板图片支持的排期。',
      at: now.subtract(const Duration(seconds: 12)),
    );
    if (role == Role.server) {
      _peers.addAll([
        _peer('办公室 PC', 'pc', online: true),
        _peer('家里的笔记本', 'nb', lastSeen: now.subtract(const Duration(days: 3))),
      ]);
    } else {
      _peers.add(_peer('Mac Studio', 'mac', online: true));
      _found.add(_device('会议室 Mac', 'mr', '192.168.2.31'));
    }
    for (var h = 8; h <= now.hour; h++) {
      _hours[h] = [0, 4, 12, 18, 15, 6, 9, 16, 21, 14, 10, 3, 2, 1, 1, 1][(h - 8) % 16];
    }
    _seedLog(now);
  }

  final Role role;
  final bool animate;
  final bool startOnline;

  /// 收到的命令，测试里检查用。
  final sent = <Request>[];

  final _link = ValueNotifier(DaemonLink.connecting);
  final _notices = StreamController<Notice>.broadcast();
  Timer? _timer;
  final _random = Random(7);
  int _tick = 0;
  int _nextLog = 0;
  int _nextPairing = 1;

  bool _sharing = true;
  bool _cursorOnPeer = false;
  Problem? _problem;
  late DateTime _since;
  ClipboardSync? _clip;
  final List<double> _rtt = List.generate(32, (i) => 0.30 + 0.05 * sin(i / 3));
  final _peers = <PeerInfo>[];
  final _found = <FoundDevice>[];
  final _hours = List.filled(24, 0);
  final _log = <LogEntry>[];
  DaemonSettings _settings = const DaemonSettings(
    name: '',
    side: ScreenSide.right,
    swapCmdCtrl: true,
    announce: true,
    clipboard: true,
    server: '',
    autostart: false,
    theme: UiTheme.system,
    accent: 0,
  );

  String get _localName => role == Role.server ? 'Mac Studio' : '办公室 PC';
  String get _peerName => role == Role.server ? '办公室 PC' : 'Mac Studio';

  @override
  ValueListenable<DaemonLink> get link => _link;

  @override
  Stream<Notice> get notices => _notices.stream;

  @override
  void start() {
    if (!startOnline) {
      _link.value = DaemonLink.offline;
      return;
    }
    goOnline();
    if (animate) _timer ??= Timer.periodic(const Duration(seconds: 1), (_) => _onTick());
  }

  /// 模拟后台连上：发送日志与状态。
  void goOnline() {
    _link.value = DaemonLink.online;
    emit(HistoryNotice(List.of(_log)));
    _emitState();
  }

  /// 模拟与后台断开。
  void goOffline() => _link.value = DaemonLink.offline;

  /// 直接推一条通知。
  void emit(Notice notice) {
    if (!_notices.isClosed) _notices.add(notice);
  }

  set problem(Problem? value) {
    _problem = value;
    _emitState();
  }

  /// 模拟对方发起配对，返回请求编号。
  int requestPairing(String peerName) {
    final id = _nextPairing++;
    emit(
      PairingNotice(
        PairingPrompt(
          id: id,
          peerName: peerName,
          fingerprint: _fp('new'),
          short: '5D21-9E0C-A4F7-3B88',
          code: '482 913',
        ),
      ),
    );
    return id;
  }

  @override
  bool send(Request request) {
    if (_link.value != DaemonLink.online) return false;
    sent.add(request);
    switch (request) {
      case SetSharingRequest(:final on):
        _sharing = on;
        _cursorOnPeer = false;
        _addLog(LogKind.link, on ? LogIcon.play : LogIcon.pause, on ? '继续共享' : '暂停共享', on ? '键鼠可以切到对方' : '键鼠留在本机');
        emit(ToastNotice(on ? '已继续共享' : '已暂停共享'));
      case UpdateSettingsRequest(:final settings):
        final side = settings.side;
        if (side != null && side != _settings.side) {
          _addLog(LogKind.link, LogIcon.layout, '$_peerName 移到本机${_sideLabel(side)}', '布局已保存');
        }
        _settings = _settings.apply(settings);
      case ForgetRequest(:final fingerprint):
        final gone = _peers.where((p) => p.fingerprint == fingerprint).toList();
        _peers.removeWhere((p) => p.fingerprint == fingerprint);
        for (final p in gone) {
          _addLog(LogKind.link, LogIcon.unlink, '已移除 ${p.name}', '下次连接需要重新配对');
          emit(ToastNotice('已移除 ${p.name}'));
        }
      case ConnectRequest(:final fingerprint):
        final device = _found.where((d) => d.fingerprint == fingerprint).firstOrNull;
        if (device != null) {
          final id = _nextPairing++;
          _pairing[id] = device;
          emit(
            PairingNotice(
              PairingPrompt(
                id: id,
                peerName: device.name,
                fingerprint: device.fingerprint,
                short: device.short,
                code: '482 913',
              ),
            ),
          );
        }
      case PairReplyRequest(:final id, :final accept):
        final device = _pairing.remove(id);
        if (accept && device != null) {
          _found.remove(device);
          _peers.insert(0, _peer(device.name, device.fingerprint, online: true, raw: true));
          _addLog(LogKind.link, LogIcon.lock, '已与 ${device.name} 配对', '指纹 ${device.short}');
          emit(PairingClosedNotice(id));
          emit(ToastNotice('已与 ${device.name} 配对'));
        } else {
          emit(PairingClosedNotice(id));
          emit(ToastNotice(accept ? '配对已结束' : '已拒绝配对'));
        }
      case ClearLogRequest():
        _log.clear();
        emit(const ToastNotice('日志已清空'));
      case HelloRequest() || OpenPermissionSettingsRequest() || OpenSettingsRequest():
        break;
    }
    _emitState();
    return true;
  }

  final _pairing = <int, FoundDevice>{};

  @override
  Future<void> dispose() async {
    _timer?.cancel();
    _link.dispose();
    await _notices.close();
  }

  void _onTick() {
    if (_link.value != DaemonLink.online) return;
    _tick++;
    if (_sharing) {
      _rtt
        ..removeAt(0)
        ..add(0.26 + _random.nextDouble() * 0.12 + (_tick % 13 == 0 ? 0.3 : 0));
      if (_tick % 6 == 0) {
        _cursorOnPeer = !_cursorOnPeer;
        if (_cursorOnPeer) _hours[DateTime.now().hour]++;
        _addLog(
          LogKind.cursor,
          LogIcon.cursor,
          _cursorOnPeer ? '光标切到 $_peerName' : '光标回到 本机',
          _cursorOnPeer ? '从本机${_sideLabel(_settings.side)}边缘' : '从对方屏幕边缘',
        );
      }
    }
    _emitState();
  }

  void _emitState() {
    if (_link.value != DaemonLink.online) return;
    emit(StateNotice(_state()));
  }

  DaemonState _state() {
    final connected = _sharing && _problem == null;
    final status = _problem != null
        ? LinkStatus.stopped
        : _sharing
        ? LinkStatus.connected
        : LinkStatus.paused;
    final now = DateTime.now();
    final today = DateTime(now.year, now.month, now.day);
    return DaemonState(
      local: Local(
        name: _localName,
        fingerprint: _fp('local'),
        short: '7AB0-5CB2-38FB-D603',
        role: role,
        dataDir: role == Role.server
            ? '~/Library/Application Support/ScreenFerry'
            : r'C:\Users\me\AppData\Roaming\ScreenFerry',
        version: '0.1.0',
      ),
      sharing: _sharing,
      problem: _problem,
      link: Link(
        status: status,
        peer: connected
            ? PeerRef(
                name: _peerName,
                fingerprint: _peers.first.fingerprint,
                short: _peers.first.short,
                addr: '192.168.2.18',
              )
            : null,
        since: connected ? _since : null,
        cursorOnPeer: connected && _cursorOnPeer,
        rttMs: connected ? List.of(_rtt) : const [],
      ),
      clipboard: _clip,
      peers: List.of(_peers),
      found: List.of(_found),
      settings: _settings,
      activity: [
        for (var h = 0; h < 24; h++)
          if (_hours[h] > 0)
            HourActivity(
              hour: today.add(Duration(hours: h)),
              switches: _hours[h],
              clips: _hours[h] ~/ 4,
              connectedSecs: h < now.hour ? 3600 : now.minute * 60,
            ),
      ],
    );
  }

  void _addLog(LogKind kind, LogIcon icon, String text, String detail, {DateTime? at}) {
    final entry = LogEntry(
      id: _nextLog++,
      kind: kind,
      icon: icon,
      text: text,
      detail: detail,
      at: at ?? DateTime.now(),
    );
    _log.insert(0, entry);
    if (_log.length > 200) _log.removeLast();
    if (at == null) emit(LogNotice(entry));
  }

  void _seedLog(DateTime now) {
    final seed = [
      (LogKind.link, LogIcon.power, 'ScreenFerry 已启动', '监听 0.0.0.0:24810', const Duration(minutes: 73)),
      (LogKind.link, LogIcon.link, '已连接 $_peerName', 'TLS 1.3 · 指纹 3F9A-12C4-77E0-B1D2', const Duration(minutes: 72)),
      (LogKind.clip, LogIcon.clipboard, '剪贴板同步到 本机', '文本 · 128 字', const Duration(minutes: 15)),
      (LogKind.cursor, LogIcon.cursor, '光标回到 本机', '快捷键 Ctrl + Option + Esc', const Duration(minutes: 7)),
      (LogKind.cursor, LogIcon.cursor, '光标切到 $_peerName', '从本机右侧边缘', const Duration(minutes: 3)),
      (LogKind.clip, LogIcon.clipboard, '剪贴板同步到 $_peerName', '文本 · 31 字', const Duration(seconds: 12)),
    ];
    for (final (kind, icon, text, detail, ago) in seed) {
      _addLog(kind, icon, text, detail, at: now.subtract(ago));
    }
  }

  static String _fp(String seed) =>
      (seed.codeUnits.fold(0, (a, b) => a * 31 + b) & 0xffffffff).toRadixString(16).padLeft(8, '0').padRight(64, 'a');

  PeerInfo _peer(String name, String seed, {bool online = false, DateTime? lastSeen, bool raw = false}) {
    final fp = raw ? seed : _fp(seed);
    return PeerInfo(name: name, fingerprint: fp, short: _short(fp), online: online, lastSeen: lastSeen);
  }

  FoundDevice _device(String name, String seed, String addr) {
    final fp = _fp(seed);
    return FoundDevice(name: name, fingerprint: fp, short: _short(fp), addr: addr, paired: false);
  }

  static String _short(String fp) => [for (var i = 0; i < 16; i += 4) fp.substring(i, i + 4).toUpperCase()].join('-');

  static String _sideLabel(ScreenSide side) => switch (side) {
    ScreenSide.left => '左侧',
    ScreenSide.right => '右侧',
    ScreenSide.top => '上方',
    ScreenSide.bottom => '下方',
  };
}
