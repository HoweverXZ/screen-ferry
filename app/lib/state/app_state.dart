// 界面状态。业务数据全部来自后台（经 Backend），命令也经 Backend 发出；
// 这里只多管导航、侧栏、提示等纯界面状态，以及“先显示、等后台确认”的乐观更新。

import 'dart:async';

import 'package:flutter/material.dart';

import '../ipc/messages.dart';
import 'backend.dart';
import 'demo_backend.dart';

export '../ipc/messages.dart';
export 'backend.dart' show Backend, DaemonLink, IpcBackend;
export 'demo_backend.dart' show DemoBackend;

enum SfPage { overview, devices, layout, log, settings, about }

extension ScreenSideLabel on ScreenSide {
  String get label => switch (this) {
    ScreenSide.left => '左侧',
    ScreenSide.right => '右侧',
    ScreenSide.top => '上方',
    ScreenSide.bottom => '下方',
  };
}

enum SwitchMode { instant, dwell }

extension LogKindLabel on LogKind {
  String get label => switch (this) {
    LogKind.cursor => '光标',
    LogKind.clip => '剪贴板',
    LogKind.link => '设备与连接',
    LogKind.file => '文件',
  };
}

extension LogIconData on LogIcon {
  IconData get data => switch (this) {
    LogIcon.power => Icons.power_settings_new_rounded,
    LogIcon.link => Icons.link_rounded,
    LogIcon.unlink => Icons.link_off_rounded,
    LogIcon.lock => Icons.lock_rounded,
    LogIcon.cursor => Icons.near_me_rounded,
    LogIcon.clipboard => Icons.content_paste_rounded,
    LogIcon.pause => Icons.pause_rounded,
    LogIcon.play => Icons.play_arrow_rounded,
    LogIcon.layout => Icons.splitscreen_rounded,
    LogIcon.radar => Icons.radar_rounded,
    LogIcon.warning => Icons.warning_amber_rounded,
    LogIcon.settings => Icons.tune_rounded,
    LogIcon.file => Icons.insert_drive_file_rounded,
  };
}

/// 外观单独通知，避免每秒的数据刷新重建整个 MaterialApp。
class Appearance extends ChangeNotifier {
  ThemeMode _mode = ThemeMode.system;
  int _accent = 0;

  ThemeMode get mode => _mode;
  int get accent => _accent;

  void apply({ThemeMode? mode, int? accent}) {
    final m = mode ?? _mode;
    final a = accent ?? _accent;
    if (m == _mode && a == _accent) return;
    _mode = m;
    _accent = a;
    notifyListeners();
  }
}

/// 底部提示。`id` 递增，同一句话连续出现也会重新弹出。
class ToastMessage {
  const ToastMessage(this.id, this.text);

  final int id;
  final String text;
}

/// 今天（本地时区）的活动汇总。
class TodayStats {
  const TodayStats({required this.switches, required this.clips, required this.connectedHours, required this.hours});

  final int switches;
  final int clips;
  final double connectedHours;

  /// 0–23 时每小时的切换次数。
  final List<int> hours;
}

/// 乐观更新等待后台确认的最长时间，超时就以后台为准。
const _pendingTimeout = Duration(seconds: 3);

class AppState extends ChangeNotifier {
  AppState({Backend? backend}) : backend = backend ?? DemoBackend() {
    _link = this.backend.link.value;
    this.backend.link.addListener(_onLink);
    _subscription = this.backend.notices.listen(_onNotice);
  }

  final Backend backend;
  late final StreamSubscription<Notice> _subscription;

  final appearance = Appearance();
  final toast = ValueNotifier<ToastMessage?>(null);
  int _toastId = 0;

  // ── 导航 ──
  SfPage page = SfPage.overview;
  bool sidebarCollapsed = false;

  // ── 与后台的连接 ──
  late DaemonLink _link;
  DaemonState? _daemon;

  DaemonLink get daemonLink => _link;

  /// 最近一次收到的状态快照；还没连上过后台时为 null。
  DaemonState? get daemon => _daemon;

  /// 已连上后台并拿到了状态，可以发命令。
  bool get online => _link == DaemonLink.online && _daemon != null;

  /// 连上过后台、现在断开了。
  bool get disconnected => _daemon != null && _link != DaemonLink.online;

  // ── 乐观更新 ──
  SettingsPatch _pending = const SettingsPatch();
  DateTime _pendingAt = DateTime.fromMillisecondsSinceEpoch(0);
  bool? _pendingSharing;
  DateTime _pendingSharingAt = DateTime.fromMillisecondsSinceEpoch(0);
  final _forgotten = <String>{};
  final _freshPeers = <String, DateTime>{};

  // ── 配对与日志 ──
  final _prompts = <PairingPrompt>[];
  final List<LogEntry> log = [];

  void start() => backend.start();

  // ── 本机 ──

  Role get role => _daemon?.local.role ?? Role.server;
  bool get isServer => role == Role.server;
  String get localName => _daemon?.local.name ?? '';
  String get localFingerprint => _daemon?.local.short ?? '';
  String get dataDir => _daemon?.local.dataDir ?? '';
  String get version => _daemon?.local.version ?? '';

  // ── 连接 ──

  bool get sharing => _pendingSharing ?? _daemon?.sharing ?? false;

  LinkStatus get status {
    if (_pendingSharing == false) return LinkStatus.paused;
    if (_pendingSharing == true && _daemon?.link.status == LinkStatus.paused) return LinkStatus.starting;
    return _daemon?.link.status ?? LinkStatus.starting;
  }

  bool get connected => status == LinkStatus.connected;
  PeerRef? get peer => connected ? _daemon?.link.peer : null;
  DateTime? get connectedAt => connected ? _daemon?.link.since : null;
  bool get cursorOnPeer => connected && (_daemon?.link.cursorOnPeer ?? false);
  Problem? get problem => _daemon?.problem;

  /// 对方的名字；没连上时取第一台已配对的设备，再没有就叫“对方”。
  String get peerName => peer?.name ?? peers.firstOrNull?.name ?? '对方';

  List<double> get rtt => connected ? (_daemon?.link.rttMs ?? const []) : const [];
  double? get latestRtt => rtt.isEmpty ? null : rtt.last;
  double? get peakRtt => rtt.isEmpty ? null : rtt.reduce((a, b) => a > b ? a : b);
  double? get avgRtt => rtt.isEmpty ? null : rtt.reduce((a, b) => a + b) / rtt.length;

  ClipboardSync? get clipboard => _daemon?.clipboard;

  // ── 设备 ──

  List<PeerInfo> get peers => [
    for (final p in _daemon?.peers ?? const <PeerInfo>[])
      if (!_forgotten.contains(p.fingerprint)) p,
  ];

  List<FoundDevice> get found => [
    for (final f in _daemon?.found ?? const <FoundDevice>[])
      if (!f.paired) f,
  ];

  /// 刚配对上的设备，用来播放卡片进场动画。
  bool isFresh(String fingerprint) {
    final at = _freshPeers[fingerprint];
    return at != null && DateTime.now().difference(at) < const Duration(seconds: 2);
  }

  // ── 设置 ──

  DaemonSettings get settings =>
      (_daemon?.settings ??
              const DaemonSettings(
                name: '',
                side: ScreenSide.right,
                swapCmdCtrl: true,
                announce: true,
                clipboard: true,
                server: '',
                autostart: false,
                theme: UiTheme.system,
                accent: 0,
              ))
          .apply(_pending);

  ScreenSide get side => settings.side;

  // ── 今日 ──

  TodayStats get today {
    final now = DateTime.now();
    final hours = List.filled(24, 0);
    var (switches, clips, secs) = (0, 0, 0);
    for (final a in _daemon?.activity ?? const <HourActivity>[]) {
      final t = a.hour.toLocal();
      if (t.year != now.year || t.month != now.month || t.day != now.day) continue;
      hours[t.hour] += a.switches;
      switches += a.switches;
      clips += a.clips;
      secs += a.connectedSecs;
    }
    return TodayStats(switches: switches, clips: clips, connectedHours: secs / 3600, hours: hours);
  }

  // ── 配对 ──

  /// 还在等待用户核对的配对请求，先来的在前。
  List<PairingPrompt> get prompts => List.unmodifiable(_prompts);

  bool isPromptOpen(int id) => _prompts.any((p) => p.id == id);

  // ── 界面状态 ──

  void update(VoidCallback change) {
    change();
    notifyListeners();
  }

  void go(SfPage target) {
    if (page == target) return;
    page = target;
    notifyListeners();
  }

  void showToast(String text) => toast.value = ToastMessage(++_toastId, text);

  // ── 命令 ──

  bool _send(Request request) {
    if (online && backend.send(request)) return true;
    showToast('还没连上后台，稍后再试');
    return false;
  }

  void setSharing(bool on) {
    if (!_send(SetSharingRequest(on))) return;
    _pendingSharing = on;
    _pendingSharingAt = DateTime.now();
    notifyListeners();
  }

  /// 修改设置：先按新值显示，等后台的状态确认。
  void updateSettings(SettingsPatch patch) {
    if (!_send(UpdateSettingsRequest(patch))) return;
    _pending = _pending.merge(patch);
    _pendingAt = DateTime.now();
    _applyAppearance();
    notifyListeners();
  }

  void setTheme(ThemeMode mode) => updateSettings(SettingsPatch(theme: UiTheme.values.byName(mode.name)));

  void setAccent(int accent) => updateSettings(SettingsPatch(accent: accent));

  void moveScreen(ScreenSide to) {
    if (to != side) updateSettings(SettingsPatch(side: to));
  }

  void forget(PeerInfo peer) {
    if (!_send(ForgetRequest(peer.fingerprint))) return;
    _forgotten.add(peer.fingerprint);
    notifyListeners();
  }

  void connectTo(FoundDevice device) {
    if (_send(ConnectRequest(device.fingerprint))) showToast('正在连接 ${device.name}…');
  }

  void replyPairing(int id, {required bool accept}) => _send(PairReplyRequest(id, accept: accept));

  void clearLog() {
    if (!_send(const ClearLogRequest())) return;
    log.clear();
    notifyListeners();
  }

  void openPermissionSettings() => _send(const OpenPermissionSettingsRequest());

  // ── 后台推来的 ──

  void _onLink() {
    _link = backend.link.value;
    notifyListeners();
  }

  void _onNotice(Notice notice) {
    switch (notice) {
      case StateNotice(:final state):
        _onState(state);
      case HistoryNotice(:final entries):
        log
          ..clear()
          ..addAll(entries);
      case LogNotice(:final entry):
        log.insert(0, entry);
        if (log.length > 500) log.removeLast();
      case PairingNotice(:final prompt):
        _prompts
          ..removeWhere((p) => p.id == prompt.id)
          ..add(prompt);
      case PairingClosedNotice(:final id):
        _prompts.removeWhere((p) => p.id == id);
      case ToastNotice(:final text):
        showToast(text);
      case WelcomeNotice():
        return;
    }
    notifyListeners();
  }

  void _onState(DaemonState state) {
    final before = _daemon?.peers.map((p) => p.fingerprint).toSet();
    final now = DateTime.now();
    if (before != null) {
      for (final p in state.peers) {
        if (!before.contains(p.fingerprint)) _freshPeers[p.fingerprint] = now;
      }
    }
    _daemon = state;

    // 后台已经确认的改动不再覆盖；等太久的以后台为准。
    _pending = now.difference(_pendingAt) > _pendingTimeout ? const SettingsPatch() : _pending.without(state.settings);
    if (_pendingSharing == state.sharing || now.difference(_pendingSharingAt) > _pendingTimeout) {
      _pendingSharing = null;
    }
    _forgotten.retainWhere((fp) => state.peers.any((p) => p.fingerprint == fp));
    _applyAppearance();
  }

  void _applyAppearance() {
    final s = settings;
    appearance.apply(mode: ThemeMode.values.byName(s.theme.name), accent: s.accent);
  }

  @override
  void dispose() {
    unawaited(_subscription.cancel());
    backend.link.removeListener(_onLink);
    unawaited(backend.dispose());
    appearance.dispose();
    toast.dispose();
    super.dispose();
  }
}

/// “12 秒前”这样的相对时间。
String timeAgo(DateTime at) {
  final s = DateTime.now().difference(at).inSeconds;
  if (s < 5) return '刚刚';
  if (s < 60) return '$s 秒前';
  if (s < 3600) return '${s ~/ 60} 分钟前';
  if (s < 86400) return '${s ~/ 3600} 小时前';
  return '${s ~/ 86400} 天前';
}

/// 已配对设备的在线情况：“在线”“5 分钟前连接过”“离线”。
String seenText(PeerInfo p) {
  if (p.online) return '在线';
  final at = p.lastSeen;
  return at == null ? '离线' : '${timeAgo(at)}连接过';
}

/// “14:32:05”这样的时刻。
String clockTime(DateTime at) {
  String two(int v) => v.toString().padLeft(2, '0');
  return '${two(at.hour)}:${two(at.minute)}:${two(at.second)}';
}

/// “1 小时 12 分”这样的时长。
String durationText(Duration d) {
  final m = d.inMinutes;
  if (m < 1) return '不到 1 分钟';
  if (m < 60) return '$m 分钟';
  return '${m ~/ 60} 小时 ${m % 60} 分';
}

/// 字节数写成“93 B”“1.2 MB”。
String sizeText(int bytes) {
  if (bytes < 1024) return '$bytes B';
  if (bytes < 1024 * 1024) return '${(bytes / 1024).toStringAsFixed(1)} KB';
  return '${(bytes / 1024 / 1024).toStringAsFixed(1)} MB';
}
