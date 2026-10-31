// 界面状态。原型阶段是模拟数据：每秒一次延迟采样、每隔几秒切换一次光标。
// 以后改由 ferry-settings（flutter_rust_bridge）把后台推来的事件写进这里，界面不用改。

import 'dart:async';
import 'dart:math';

import 'package:flutter/material.dart';

enum SfPage { overview, devices, layout, log, settings, about }

enum ScreenSide { left, right, top, bottom }

extension ScreenSideLabel on ScreenSide {
  String get label => switch (this) {
    ScreenSide.left => '左侧',
    ScreenSide.right => '右侧',
    ScreenSide.top => '上方',
    ScreenSide.bottom => '下方',
  };
}

enum SwitchMode { instant, dwell }

enum LogKind { cursor, clip, link }

extension LogKindLabel on LogKind {
  String get label => switch (this) {
    LogKind.cursor => '光标',
    LogKind.clip => '剪贴板',
    LogKind.link => '设备与连接',
  };
}

class LogEntry {
  LogEntry(this.id, this.kind, this.icon, this.text, this.detail, this.at);

  final int id;
  final LogKind kind;
  final IconData icon;
  final String text;
  final String detail;
  final DateTime at;
}

class Device {
  Device({
    required this.id,
    required this.name,
    required this.fingerprint,
    required this.os,
    required this.address,
    this.laptop = false,
    this.online = false,
    this.lastSeen = '',
  });

  final String id;
  final String name;
  final String fingerprint;
  final String os;
  final String address;
  final bool laptop;
  bool online;
  String lastSeen;

  /// 刚配对的时刻，用来播放卡片进场动画。
  DateTime? pairedAt;

  String get seen => online ? '在线 · $address' : lastSeen;
}

/// 外观单独通知，避免每秒的数据刷新重建整个 MaterialApp。
class Appearance extends ChangeNotifier {
  ThemeMode _mode = ThemeMode.system;
  int _accent = 0;

  ThemeMode get mode => _mode;
  int get accent => _accent;

  set mode(ThemeMode value) {
    _mode = value;
    notifyListeners();
  }

  set accent(int value) {
    _accent = value;
    notifyListeners();
  }
}

/// 底部提示。`id` 递增，同一句话连续出现也会重新弹出。
class ToastMessage {
  const ToastMessage(this.id, this.text);

  final int id;
  final String text;
}

class AppState extends ChangeNotifier {
  AppState() {
    final now = DateTime.now();
    connectedAt = now.subtract(const Duration(hours: 1, minutes: 12, seconds: 14));
    clipAt = now.subtract(const Duration(seconds: 12));
    _seedLog(now);
  }

  final appearance = Appearance();
  final toast = ValueNotifier<ToastMessage?>(null);

  // ── 导航 ──
  SfPage page = SfPage.overview;
  bool sidebarCollapsed = false;

  // ── 本机 ──
  final String localName = 'Mac Studio';
  final String localFingerprint = '7AB0-5CB2-38FB-D603';
  final String dataDir = '~/Library/Application Support/ScreenFerry';

  // ── 连接 ──
  bool sharing = true;
  bool cursorOnPeer = false;
  final String peerName = '办公室 PC';
  final String peerOs = 'Windows 11';
  final String peerAddress = '192.168.2.18';
  late DateTime connectedAt;
  final List<double> rtt = List.generate(32, (i) => 0.30 + 0.05 * sin(i / 3));

  double get latestRtt => rtt.last;
  double get peakRtt => rtt.reduce(max);
  double get avgRtt => rtt.reduce((a, b) => a + b) / rtt.length;

  // ── 剪贴板 ──
  late DateTime clipAt;
  final String clipPreview = '周三例会纪要：确认屏幕布局、配对流程，以及剪贴板图片支持的排期。';
  final String clipMeta = '文本 · 31 字';

  // ── 今日 ──
  int switchesToday = 128;
  int clipsToday = 36;
  final double sharedHours = 6.2;
  final List<int> hours = [0, 0, 0, 0, 0, 0, 0, 0, 4, 12, 18, 15, 6, 9, 16, 21, 14, 10, 3, 0, 0, 0, 0, 0];

  // ── 开关 ──
  bool syncClipboard = true;
  bool swapCommandControl = true;
  bool announce = true;
  bool launchAtLogin = false;
  bool holdWhileDragging = true;
  double clipboardLimitMb = 32;

  // ── 布局 ──
  ScreenSide side = ScreenSide.right;
  SwitchMode switchMode = SwitchMode.instant;

  // ── 设备 ──
  final List<Device> paired = [
    Device(
      id: 'pc',
      name: '办公室 PC',
      fingerprint: '3F9A-12C4-77E0-B1D2',
      os: 'Windows 11',
      address: '192.168.2.18',
      online: true,
    ),
    Device(
      id: 'nb',
      name: '家里的笔记本',
      fingerprint: 'C81E-0B57-94AD-2F60',
      os: 'Windows 10',
      address: '192.168.1.7',
      laptop: true,
      lastSeen: '3 天前连接过',
    ),
  ];
  final List<Device> found = [
    Device(id: 'mr', name: '会议室 PC', fingerprint: '5D21-9E0C-A4F7-3B88', os: 'Windows 11', address: '192.168.2.31'),
  ];

  // ── 日志 ──
  final List<LogEntry> log = [];
  int _nextLogId = 0;

  Timer? _timer;
  final _random = Random(7);
  int _tick = 0;
  int _toastId = 0;

  void start() {
    _timer ??= Timer.periodic(const Duration(seconds: 1), (_) => _onTick());
  }

  void _onTick() {
    _tick++;
    if (sharing) {
      rtt
        ..removeAt(0)
        ..add(0.26 + _random.nextDouble() * 0.12 + (_tick % 13 == 0 ? 0.3 : 0));
      if (_tick % 6 == 0) {
        cursorOnPeer = !cursorOnPeer;
        switchesToday++;
        hours[DateTime.now().hour]++;
        addLog(
          LogKind.cursor,
          Icons.near_me_rounded,
          cursorOnPeer ? '光标切到 $peerName' : '光标回到 本机',
          cursorOnPeer ? '从本机${side.label}边缘' : '从对方屏幕边缘',
          notify: false,
        );
      }
    }
    notifyListeners();
  }

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

  void setSharing(bool on) {
    sharing = on;
    if (!on) cursorOnPeer = false;
    addLog(
      LogKind.link,
      on ? Icons.play_arrow_rounded : Icons.pause_rounded,
      on ? '继续共享' : '暂停共享',
      on ? '键鼠可以切到对方' : '键鼠留在本机',
      notify: false,
    );
    notifyListeners();
    showToast(on ? '已继续共享' : '已暂停共享');
  }

  void addLog(LogKind kind, IconData icon, String text, String detail, {bool notify = true}) {
    log.insert(0, LogEntry(_nextLogId++, kind, icon, text, detail, DateTime.now()));
    if (log.length > 200) log.removeLast();
    if (notify) notifyListeners();
  }

  void clearLog() {
    log.clear();
    notifyListeners();
    showToast('日志已清空');
  }

  void moveScreen(ScreenSide to) {
    if (to == side) return;
    side = to;
    addLog(LogKind.link, Icons.splitscreen_rounded, '$peerName 移到本机${to.label}', '布局已保存', notify: false);
    notifyListeners();
  }

  void removeDevice(Device device) {
    paired.remove(device);
    addLog(LogKind.link, Icons.link_off_rounded, '已移除 ${device.name}', '下次连接需要重新配对', notify: false);
    notifyListeners();
    showToast('已移除 ${device.name}');
  }

  void completePairing(Device device) {
    found.remove(device);
    device
      ..online = true
      ..lastSeen = ''
      ..pairedAt = DateTime.now();
    paired.insert(0, device);
    addLog(LogKind.link, Icons.lock_rounded, '已与 ${device.name} 配对', '指纹 ${device.fingerprint}', notify: false);
    notifyListeners();
    showToast('已与 ${device.name} 配对');
  }

  void _seedLog(DateTime now) {
    final seed = [
      (
        LogKind.link,
        Icons.power_settings_new_rounded,
        'ScreenFerry 已启动',
        '监听 0.0.0.0:24810',
        const Duration(minutes: 73),
      ),
      (
        LogKind.link,
        Icons.link_rounded,
        '已连接 $peerName',
        'TLS 1.3 · 指纹 3F9A-12C4-77E0-B1D2',
        const Duration(minutes: 72, seconds: 14),
      ),
      (LogKind.link, Icons.wifi_tethering_rounded, '在局域网里发现 会议室 PC', '192.168.2.31 · 未配对', const Duration(minutes: 30)),
      (LogKind.clip, Icons.content_paste_rounded, '剪贴板同步到 本机', '文本 · 128 字', const Duration(minutes: 15)),
      (LogKind.cursor, Icons.near_me_rounded, '光标回到 本机', '快捷键 Ctrl + Option + Esc', const Duration(minutes: 7)),
      (LogKind.cursor, Icons.near_me_rounded, '光标切到 $peerName', '从本机右侧边缘', const Duration(minutes: 3, seconds: 10)),
      (LogKind.clip, Icons.content_paste_rounded, '剪贴板同步到 $peerName', '文本 · 31 字', const Duration(seconds: 12)),
    ];
    for (final (kind, icon, text, detail, ago) in seed) {
      log.insert(0, LogEntry(_nextLogId++, kind, icon, text, detail, now.subtract(ago)));
    }
  }

  @override
  void dispose() {
    _timer?.cancel();
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
  return '${s ~/ 3600} 小时前';
}

/// “14:32:05”这样的时刻。
String clockTime(DateTime at) {
  String two(int v) => v.toString().padLeft(2, '0');
  return '${two(at.hour)}:${two(at.minute)}:${two(at.second)}';
}
