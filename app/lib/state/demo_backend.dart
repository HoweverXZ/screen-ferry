// 演示用的假后台：不连真实的后台，按协议推送模拟的状态、回应命令。
// 供 widget 测试使用，也方便单独看界面设计（例如 `flutter run --dart-define=SF_DEMO=true`）。

import 'dart:async';
import 'dart:math';

import 'package:flutter/foundation.dart';

import '../ipc/messages.dart';
import 'arrangement.dart';
import 'backend.dart';

class DemoBackend implements Backend {
  /// [paired] 为假时没有任何已配对的设备。
  DemoBackend({
    this.role = Role.server,
    String? platform,
    this.animate = true,
    this.startOnline = true,
    bool paired = true,
    this._problem,
    ClipboardSync? clipboard,
  }) : platform = platform ?? (role == Role.server ? 'macos' : 'windows') {
    final now = DateTime.now();
    _since = now.subtract(const Duration(hours: 1, minutes: 12, seconds: 14));
    _clip =
        clipboard ??
        ClipboardSync(
          toPeer: true,
          peer: _peerName,
          chars: 31,
          bytes: 93,
          preview: '周三例会纪要：确认屏幕布局、配对流程，以及剪贴板图片支持的排期。',
          at: now.subtract(const Duration(seconds: 12)),
        );
    if (role == Role.server) {
      if (paired) {
        // 办公室 PC 和书房 PC 在右侧（书房 PC 没有单独安排，跟着默认的一侧排在后面），笔记本在左侧。
        _peers.addAll([
          _peer('办公室 PC', 'pc', online: true),
          _peer('书房 PC', 'study', online: true),
          _peer('家里的笔记本', 'nb', lastSeen: now.subtract(const Duration(days: 3))),
        ]);
        _settings = _settings
            .apply(
              SettingsPatch(layout: [Placement(_fp('pc'), ScreenSide.right), Placement(_fp('nb'), ScreenSide.left)]),
            )
            // 办公室 PC 单独设置过：滚动反向、F13 当 Print Screen。
            .withDevice(
              DeviceSettings(
                fingerprint: _fp('pc'),
                reverseVertical: true,
                keys: const [KeyMapping(0x00070068, 0x00070046)],
              ),
            );
      }
    } else {
      if (paired) _peers.add(_peer('Mac Studio', 'mac', online: true));
      _found.add(_device('会议室 Mac', 'mr', '192.168.2.31'));
    }
    for (var h = 8; h <= now.hour; h++) {
      _hours[h] = [0, 4, 12, 18, 15, 6, 9, 16, 21, 14, 10, 3, 2, 1, 1, 1][(h - 8) % 16];
    }
    _seedLog(now);
  }

  final Role role;

  /// 本机系统。不给时按角色推断：服务端是 Mac，客户端是 Windows。
  final String platform;
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

  /// 光标所在的设备；在本机时为 null。
  String? _cursor;

  /// 最近连上（或光标最近去过）的设备。
  String? _recent;
  Problem? _problem;
  late DateTime _since;
  ClipboardSync? _clip;
  final List<double> _rtt = List.generate(32, (i) => 0.30 + 0.05 * sin(i / 3));
  final _peers = <PeerInfo>[];
  final _found = <FoundDevice>[];
  final _hours = List.filled(24, 0);
  final _log = <LogEntry>[];

  /// 详细记录从什么时候开着。
  DateTime? _verboseSince;

  DaemonSettings _settings = const DaemonSettings(
    name: '',
    side: ScreenSide.right,
    announce: true,
    clipboard: true,
    server: '',
    autostart: false,
    theme: UiTheme.system,
    accent: 0,
  );

  String get _localName => role == Role.server ? 'Mac Studio' : '办公室 PC';

  /// 连接摘要指向的那台：光标所在的，否则最近连上的，再否则第一台在线的。
  PeerInfo? get _linkPeer {
    final online = _peers.where((p) => p.online).toList();
    for (final fp in [_cursor, _recent]) {
      final p = online.where((p) => p.fingerprint == fp).firstOrNull;
      if (p != null) return p;
    }
    return online.firstOrNull;
  }

  String get _peerName =>
      _shown(_linkPeer) ?? _shown(_peers.firstOrNull) ?? (role == Role.server ? '办公室 PC' : 'Mac Studio');

  /// 一台设备显示的名字：起过名字就用起的，和后台一样。
  String? _shown(PeerInfo? p) {
    if (p == null) return null;
    final alias = _settings.device(p.fingerprint).alias.trim();
    return alias.isEmpty ? p.name : alias;
  }

  String _nameOf(String fingerprint) => _shown(_peers.where((p) => p.fingerprint == fingerprint).firstOrNull) ?? '对方';

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

  /// 模拟光标切到某台设备；null 为回到本机。
  set cursor(String? fingerprint) {
    _cursor = fingerprint;
    if (fingerprint != null) _recent = fingerprint;
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
        _cursor = null;
        _addLog(LogKind.link, on ? LogIcon.play : LogIcon.pause, on ? '继续共享' : '暂停共享', on ? '键鼠可以切到对方' : '键鼠留在本机');
        emit(ToastNotice(on ? '已继续共享' : '已暂停共享'));
      case UpdateSettingsRequest(:final settings):
        final before = Arrangement.of(_peers, _settings);
        _settings = _settings.apply(settings);
        _logLayout(before, Arrangement.of(_peers, _settings));
      case SetDeviceRequest(:final device):
        _settings = _settings.withDevice(device);
      // 演示：地址里带 9.9.9.9 的连不上，其余都成。
      case ConnectAddressRequest(:final address):
        emit(DialedNotice(address: address, invite: false, error: _demoDial(address)));
      case InviteRequest(:final address):
        emit(DialedNotice(address: address, invite: true, error: _demoDial(address)));
      case QuitRequest():
        emit(const QuitNotice());
      // 演示：运行日志第一次给一段，之后没有新的。
      case ReadRunLogRequest(:final since):
        emit(
          RunLogNotice(
            lines: since == null ? _demoRunLog() : const [],
            next: 2097152,
            fresh: since == null,
            bytes: 2097152,
            linesTotal: 18241,
          ),
        );
      case ExportDiagnosticsRequest():
        emit(
          DiagnosticsExportedNotice(
            path: platform == 'macos'
                ? '/Users/me/Downloads/ScreenFerry-诊断-20261102-0624.txt'
                : r'C:\Users\me\Downloads\ScreenFerry-诊断-20261102-0624.txt',
            bytes: 421888,
          ),
        );
      case CopyDiagnosticsRequest():
        emit(const DiagnosticsSummaryNotice('ScreenFerry 诊断摘要\n\n版本  0.1.0\n'));
      case SetVerboseRequest(:final on):
        _verboseSince = on ? DateTime.now() : null;
        _emitState();
      case RevealDiagnosticsRequest() || OpenLogsFolderRequest():
        break;
      case OpenPeerSettingsRequest(:final fingerprint):
        emit(ToastNotice('已在 ${_nameOf(fingerprint)} 上打开设置，把光标移过去就能改'));
      case ForgetRequest(:final fingerprint):
        final gone = _peers.where((p) => p.fingerprint == fingerprint).toList();
        _peers.removeWhere((p) => p.fingerprint == fingerprint);
        if (_cursor == fingerprint) _cursor = null;
        // 它的单独设置也不再需要。
        _settings = _settings.withDevice(DeviceSettings(fingerprint: fingerprint));
        // 它的位置安排也不再需要。
        if (_settings.layout.any((p) => p.fingerprint == fingerprint)) {
          _settings = _settings.apply(
            SettingsPatch(layout: [..._settings.layout.where((p) => p.fingerprint != fingerprint)]),
          );
        }
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
      case SendFilesRequest(:final paths, :final to):
        final name = to == null ? _peerName : _nameOf(to);
        _addLog(LogKind.file, LogIcon.file, '发送 ${paths.length} 项到 $name', '演示模式不会真的发送');
        emit(ToastNotice('开始发给 $name'));
      case HelloRequest() || OpenPermissionSettingsRequest() || OpenSettingsRequest():
        break;
    }
    _emitState();
    return true;
  }

  final _pairing = <int, FoundDevice>{};

  static DialError? _demoDial(String address) => address.contains('9.9.9.9') ? DialError.unreachable : null;

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
      if (_tick % 6 == 0) _moveCursor();
    }
    _emitState();
  }

  /// 光标轮流去每台在线的设备，中间回到本机一次。
  void _moveCursor() {
    final online = _peers.where((p) => p.online).toList();
    if (online.isEmpty) return;
    if (_cursor != null) {
      final from = _nameOf(_cursor!);
      _cursor = null;
      _addLog(LogKind.cursor, LogIcon.cursor, '光标回到 本机', '从 $from 的屏幕边缘');
      return;
    }
    final last = online.indexWhere((p) => p.fingerprint == _recent);
    final next = online[(last + 1) % online.length];
    _cursor = next.fingerprint;
    _recent = next.fingerprint;
    _hours[DateTime.now().hour]++;
    final side = role == Role.server ? Arrangement.of(_peers, _settings).placeOf(next.fingerprint)?.side : null;
    _addLog(LogKind.cursor, LogIcon.cursor, '光标切到 ${next.name}', side == null ? '从本机屏幕边缘' : '从本机${_sideLabel(side)}边缘');
  }

  /// 布局改了：哪台换了一侧，或同一侧换了先后。
  void _logLayout(Arrangement before, Arrangement after) {
    if (before.sameAs(after)) return;
    final moved = [
      for (final p in after.all)
        if (before.placeOf(p.fingerprint)?.side != after.placeOf(p.fingerprint)?.side) p,
    ];
    if (moved.isEmpty) {
      _addLog(LogKind.link, LogIcon.layout, '调整了同一侧设备的先后', '布局已保存');
      return;
    }
    for (final p in moved) {
      _addLog(LogKind.link, LogIcon.layout, '${p.name} 移到本机${_sideLabel(after.placeOf(p.fingerprint)!.side)}', '布局已保存');
    }
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
    final linkPeer = connected ? _linkPeer : null;
    final arrangement = Arrangement.of(_peers, _settings);
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
        platform: platform,
        naturalScrolling: platform == 'macos' ? true : null,
        addresses: role == Role.server ? const ['192.168.2.12'] : const ['192.168.2.18'],
        invitePort: role == Role.server ? null : 24811,
        system: platform == 'macos' ? 'macOS 15.6 · arm64' : 'Windows 11（10.0.26100.4652）· x64',
        verboseSince: _verboseSince,
      ),
      sharing: _sharing,
      problem: _problem,
      link: Link(
        status: linkPeer == null && status == LinkStatus.connected
            ? (role == Role.server ? LinkStatus.waiting : LinkStatus.searching)
            : status,
        peer: linkPeer == null
            ? null
            : PeerRef(
                name: linkPeer.name,
                fingerprint: linkPeer.fingerprint,
                short: linkPeer.short,
                addr: '192.168.2.${18 + _peers.indexOf(linkPeer)}',
              ),
        since: linkPeer != null ? _since : null,
        cursorOnPeer: linkPeer != null && _cursor == linkPeer.fingerprint,
        rttMs: linkPeer != null ? List.of(_rtt) : const [],
      ),
      // 和后台一样，按对方现在的名字显示最近一次同步。
      clipboard: switch (_clip) {
        final c? => ClipboardSync(
          toPeer: c.toPeer,
          peer: _peerName,
          chars: c.chars,
          bytes: c.bytes,
          preview: c.preview,
          rich: c.rich,
          image: c.image,
          at: c.at,
        ),
        null => null,
      },
      peers: [
        for (final p in _peers)
          PeerInfo(
            name: p.name,
            fingerprint: p.fingerprint,
            short: p.short,
            online: p.online,
            lastSeen: p.lastSeen,
            side: role == Role.server ? arrangement.placeOf(p.fingerprint)?.side : null,
            cursor: connected && p.online && _cursor == p.fingerprint,
            alias: _settings.device(p.fingerprint).alias,
            // 演示里对方总是和本机不同的系统。
            platform: platform == 'macos' ? 'windows' : 'macos',
            addr: switch ((p.online || p.lastSeen != null, role)) {
              (false, _) => '',
              (true, Role.server) => '192.168.2.${18 + _peers.indexOf(p)}',
              (true, Role.client) => '192.168.2.12',
            },
          ),
      ],
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

  static List<RunLogLine> _demoRunLog() => const [
    RunLogLine(at: '06:12:03', level: RunLogLevel.info, text: 'ScreenFerry 0.1.0 已启动 · Mac Studio · 键鼠在本机'),
    RunLogLine(at: '06:12:03', level: RunLogLevel.info, text: '等待客户端连接 addr=0.0.0.0:24810'),
    RunLogLine(at: '06:12:04', level: RunLogLevel.info, text: '客户端已连接 peer=办公室 PC'),
    RunLogLine(at: '06:12:04', level: RunLogLevel.info, text: '已连接 办公室 PC TLS 1.3 · 192.168.2.18'),
    RunLogLine(at: '06:15:21', level: RunLogLevel.info, text: '光标切到 办公室 PC 从本机右侧边缘'),
    RunLogLine(at: '06:15:40', level: RunLogLevel.info, text: '剪贴板同步到 办公室 PC 文本 · 31 字'),
    RunLogLine(
      at: '06:16:00',
      level: RunLogLevel.info,
      text: '往返延迟（过去 1 分钟 590 次）：最小 0.21 ms · 平均 0.32 ms · 最大 0.61 ms',
    ),
    RunLogLine(at: '06:20:02', level: RunLogLevel.warn, text: '连接中断：网络错误 · Connection reset by peer'),
    RunLogLine(at: '06:20:04', level: RunLogLevel.error, text: '心跳超时：对方长时间没有响应'),
    RunLogLine(at: '06:20:06', level: RunLogLevel.info, text: '已连接 办公室 PC TLS 1.3 · 192.168.2.18'),
  ];
}
