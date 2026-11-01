// 本机通信协议的消息类型，与 crates/ferry-ipc/src/message.rs 一一对应，说明见 docs/ipc.md。
// 两边靠 test/fixtures/ipc_samples.json 中的样例保持一致；改协议时两边一起改。
// 不认识的字段忽略，不认识的取值退回到合理的缺省值。

/// 协议版本。
const ipcVersion = 1;

typedef Json = Map<String, Object?>;

T _enum<T extends Enum>(List<T> values, Object? name, T fallback) => _enumOrNull(values, name) ?? fallback;

T? _enumOrNull<T extends Enum>(List<T> values, Object? name) {
  for (final v in values) {
    if (v.name == name) return v;
  }
  return null;
}

String _str(Object? v) => v is String ? v : '';
int _int(Object? v) => v is num ? v.toInt() : 0;
bool _bool(Object? v) => v == true;
bool _sameList<T>(List<T> a, List<T> b) =>
    a.length == b.length && [for (var i = 0; i < a.length; i++) a[i] == b[i]].every((x) => x);
DateTime? _time(Object? v) => v is num ? DateTime.fromMillisecondsSinceEpoch(v.toInt()) : null;
List<Json> _list(Object? v) => v is List ? v.whereType<Json>().toList() : const [];

/// snake_case 的取值转成 Dart 枚举名（`port_in_use` → `portInUse`）。
String _camel(Object? v) {
  final parts = _str(v).split('_');
  return parts.first + parts.skip(1).map((p) => p.isEmpty ? p : p[0].toUpperCase() + p.substring(1)).join();
}

// ── 枚举 ─────────────────────────────────────

enum Role { server, client }

enum ScreenSide { left, right, top, bottom }

/// 设置界面的配色，取值与 Flutter 的 ThemeMode 同名。
enum UiTheme { system, light, dark }

enum LinkStatus { paused, starting, waiting, searching, connecting, connected, stopped }

enum ProblemKind { permission, portInUse, other }

enum LogKind { cursor, clip, link, file }

enum LogIcon { power, link, unlink, lock, cursor, clipboard, pause, play, layout, radar, warning, settings, file }

// ── 状态 ─────────────────────────────────────

/// 后台的完整状态快照。
class DaemonState {
  const DaemonState({
    required this.local,
    required this.sharing,
    required this.problem,
    required this.link,
    required this.clipboard,
    required this.peers,
    required this.found,
    required this.settings,
    required this.activity,
  });

  factory DaemonState.fromJson(Json j) => DaemonState(
    local: Local.fromJson(j['local'] as Json? ?? const {}),
    sharing: _bool(j['sharing']),
    problem: j['problem'] is Json ? Problem.fromJson(j['problem'] as Json) : null,
    link: Link.fromJson(j['link'] as Json? ?? const {}),
    clipboard: j['clipboard'] is Json ? ClipboardSync.fromJson(j['clipboard'] as Json) : null,
    peers: [for (final p in _list(j['peers'])) PeerInfo.fromJson(p)],
    found: [for (final f in _list(j['found'])) FoundDevice.fromJson(f)],
    settings: DaemonSettings.fromJson(j['settings'] as Json? ?? const {}),
    activity: [for (final a in _list(j['activity'])) HourActivity.fromJson(a)],
  );

  final Local local;

  /// 用户没有暂停共享。
  final bool sharing;
  final Problem? problem;
  final Link link;
  final ClipboardSync? clipboard;
  final List<PeerInfo> peers;
  final List<FoundDevice> found;
  final DaemonSettings settings;
  final List<HourActivity> activity;
}

class Local {
  const Local({
    required this.name,
    required this.fingerprint,
    required this.short,
    required this.role,
    required this.dataDir,
    required this.version,
    this.port,
    this.platform = '',
  });

  factory Local.fromJson(Json j) => Local(
    name: _str(j['name']),
    fingerprint: _str(j['fingerprint']),
    short: _str(j['short']),
    role: _enum(Role.values, j['role'], Role.server),
    dataDir: _str(j['data_dir']),
    version: _str(j['version']),
    port: j['port'] is num ? _int(j['port']) : null,
    platform: _str(j['platform']),
  );

  final String name;
  final String fingerprint;
  final String short;
  final Role role;
  final String dataDir;
  final String version;

  /// 服务端正在监听的端口；客户端、或还没开始监听时为 null。
  final int? port;

  /// 本机系统：`macos`、`windows` 等。旧版后台不报，为空。
  final String platform;
}

class Problem {
  const Problem({required this.kind, required this.message});

  factory Problem.fromJson(Json j) =>
      Problem(kind: _enum(ProblemKind.values, _camel(j['kind']), ProblemKind.other), message: _str(j['message']));

  final ProblemKind kind;
  final String message;
}

class Link {
  const Link({
    required this.status,
    required this.peer,
    required this.since,
    required this.cursorOnPeer,
    required this.rttMs,
  });

  factory Link.fromJson(Json j) => Link(
    status: _enum(LinkStatus.values, j['status'], LinkStatus.starting),
    peer: j['peer'] is Json ? PeerRef.fromJson(j['peer'] as Json) : null,
    since: _time(j['since']),
    cursorOnPeer: _bool(j['cursor_on_peer']),
    rttMs: [
      for (final v in (j['rtt_ms'] is List ? j['rtt_ms'] as List : const []))
        if (v is num) v.toDouble(),
    ],
  );

  final LinkStatus status;
  final PeerRef? peer;
  final DateTime? since;
  final bool cursorOnPeer;

  /// 每秒一个的平均往返延迟（毫秒），先旧后新。
  final List<double> rttMs;
}

/// 连接中的对方。
class PeerRef {
  const PeerRef({required this.name, required this.fingerprint, required this.short, required this.addr});

  factory PeerRef.fromJson(Json j) => PeerRef(
    name: _str(j['name']),
    fingerprint: _str(j['fingerprint']),
    short: _str(j['short']),
    addr: _str(j['addr']),
  );

  final String name;
  final String fingerprint;
  final String short;
  final String addr;
}

class ClipboardSync {
  const ClipboardSync({
    required this.toPeer,
    required this.peer,
    required this.chars,
    required this.bytes,
    required this.preview,
    required this.at,
  });

  factory ClipboardSync.fromJson(Json j) => ClipboardSync(
    toPeer: _bool(j['to_peer']),
    peer: _str(j['peer']),
    chars: j['chars'] is num ? _int(j['chars']) : null,
    bytes: _int(j['bytes']),
    preview: _str(j['preview']),
    at: _time(j['at']) ?? DateTime.fromMillisecondsSinceEpoch(0),
  );

  final bool toPeer;
  final String peer;

  /// 纯文本的字符数；不是纯文本时为 null。
  final int? chars;
  final int bytes;
  final String preview;
  final DateTime at;
}

class PeerInfo {
  const PeerInfo({
    required this.name,
    required this.fingerprint,
    required this.short,
    required this.online,
    required this.lastSeen,
    this.side,
    this.cursor = false,
  });

  factory PeerInfo.fromJson(Json j) => PeerInfo(
    name: _str(j['name']),
    fingerprint: _str(j['fingerprint']),
    short: _str(j['short']),
    online: _bool(j['online']),
    lastSeen: _time(j['last_seen']),
    side: _enumOrNull(ScreenSide.values, j['side']),
    cursor: _bool(j['cursor']),
  );

  final String name;
  final String fingerprint;
  final String short;
  final bool online;

  /// 本次运行中最后一次断开的时刻；没连过为 null。
  final DateTime? lastSeen;

  /// 服务端：这台设备实际在本机哪一侧；客户端为 null。
  final ScreenSide? side;

  /// 光标正在这台设备上。
  final bool cursor;
}

/// 单独安排过位置的一台设备：放在本机哪一侧。
class Placement {
  const Placement(this.fingerprint, this.side);

  factory Placement.fromJson(Json j) =>
      Placement(_str(j['fingerprint']), _enum(ScreenSide.values, j['side'], ScreenSide.right));

  /// 设备的证书指纹，64 位十六进制。
  final String fingerprint;
  final ScreenSide side;

  Json toJson() => {'fingerprint': fingerprint, 'side': side.name};

  @override
  bool operator ==(Object other) => other is Placement && other.fingerprint == fingerprint && other.side == side;

  @override
  int get hashCode => Object.hash(fingerprint, side);

  @override
  String toString() => 'Placement($fingerprint, ${side.name})';
}

/// 局域网里发现的一台服务端。
class FoundDevice {
  const FoundDevice({
    required this.name,
    required this.fingerprint,
    required this.short,
    required this.addr,
    required this.paired,
  });

  factory FoundDevice.fromJson(Json j) => FoundDevice(
    name: _str(j['name']),
    fingerprint: _str(j['fingerprint']),
    short: _str(j['short']),
    addr: _str(j['addr']),
    paired: _bool(j['paired']),
  );

  final String name;
  final String fingerprint;
  final String short;
  final String addr;
  final bool paired;
}

/// 用户可以修改的设置。
class DaemonSettings {
  const DaemonSettings({
    required this.name,
    required this.side,
    this.layout = const [],
    required this.swapCmdCtrl,
    required this.announce,
    required this.clipboard,
    required this.server,
    required this.autostart,
    required this.theme,
    required this.accent,
  });

  factory DaemonSettings.fromJson(Json j) => DaemonSettings(
    name: _str(j['name']),
    side: _enum(ScreenSide.values, j['side'], ScreenSide.right),
    layout: [for (final p in _list(j['layout'])) Placement.fromJson(p)],
    swapCmdCtrl: _bool(j['swap_cmd_ctrl']),
    announce: _bool(j['announce']),
    clipboard: _bool(j['clipboard']),
    server: _str(j['server']),
    autostart: _bool(j['autostart']),
    theme: _enum(UiTheme.values, j['theme'], UiTheme.system),
    accent: _int(j['accent']),
  );

  final String name;

  /// 没有在 [layout] 里单独安排的设备放在本机哪一侧，排在已安排的后面。
  final ScreenSide side;

  /// 单独安排过位置的设备，按先后排列：同一侧的几台按这个先后平分那条边
  /// （左右两侧从上到下，上下两侧从左到右）。
  final List<Placement> layout;
  final bool swapCmdCtrl;
  final bool announce;
  final bool clipboard;
  final String server;
  final bool autostart;
  final UiTheme theme;
  final int accent;

  /// 叠加一份补丁，界面在后台确认之前先按它显示。
  DaemonSettings apply(SettingsPatch p) => DaemonSettings(
    name: p.name ?? name,
    side: p.side ?? side,
    layout: p.layout ?? layout,
    swapCmdCtrl: p.swapCmdCtrl ?? swapCmdCtrl,
    announce: p.announce ?? announce,
    clipboard: p.clipboard ?? clipboard,
    server: p.server ?? server,
    autostart: p.autostart ?? autostart,
    theme: p.theme ?? theme,
    accent: p.accent ?? accent,
  );
}

/// 一小时内的活动。
class HourActivity {
  const HourActivity({required this.hour, required this.switches, required this.clips, required this.connectedSecs});

  factory HourActivity.fromJson(Json j) => HourActivity(
    hour: DateTime.fromMillisecondsSinceEpoch(_int(j['hour']) * 1000),
    switches: _int(j['switches']),
    clips: _int(j['clips']),
    connectedSecs: _int(j['connected_secs']),
  );

  /// 这一小时开始的时刻。
  final DateTime hour;
  final int switches;
  final int clips;
  final int connectedSecs;
}

class LogEntry {
  const LogEntry({
    required this.id,
    required this.kind,
    required this.icon,
    required this.text,
    required this.detail,
    required this.at,
  });

  factory LogEntry.fromJson(Json j) => LogEntry(
    id: _int(j['id']),
    kind: _enum(LogKind.values, j['kind'], LogKind.link),
    icon: _enum(LogIcon.values, j['icon'], LogIcon.settings),
    text: _str(j['text']),
    detail: _str(j['detail']),
    at: _time(j['at']) ?? DateTime.fromMillisecondsSinceEpoch(0),
  );

  final int id;
  final LogKind kind;
  final LogIcon icon;
  final String text;
  final String detail;
  final DateTime at;
}

/// 请用户核对的配对码。
class PairingPrompt {
  const PairingPrompt({
    required this.id,
    required this.peerName,
    required this.fingerprint,
    required this.short,
    required this.code,
  });

  factory PairingPrompt.fromJson(Json j) => PairingPrompt(
    id: _int(j['id']),
    peerName: _str(j['peer_name']),
    fingerprint: _str(j['fingerprint']),
    short: _str(j['short']),
    code: _str(j['code']),
  );

  final int id;
  final String peerName;
  final String fingerprint;
  final String short;

  /// 例如 `482 913`。
  final String code;

  /// 去掉空格后的 6 位数字。
  String get digits => code.replaceAll(RegExp(r'\s'), '');
}

// ── 后台 → 界面 ──────────────────────────────

sealed class Notice {
  const Notice();

  /// 不认识的 `type` 返回 null。
  static Notice? fromJson(Json j) => switch (j['type']) {
    'welcome' => WelcomeNotice(_int(j['version'])),
    'state' => StateNotice(DaemonState.fromJson(j['state'] as Json? ?? const {})),
    'history' => HistoryNotice([for (final e in _list(j['entries'])) LogEntry.fromJson(e)]),
    'log' => LogNotice(LogEntry.fromJson(j['entry'] as Json? ?? const {})),
    'pairing' => PairingNotice(PairingPrompt.fromJson(j['prompt'] as Json? ?? const {})),
    'pairing_closed' => PairingClosedNotice(_int(j['id'])),
    'toast' => ToastNotice(_str(j['text'])),
    _ => null,
  };
}

class WelcomeNotice extends Notice {
  const WelcomeNotice(this.version);
  final int version;
}

class StateNotice extends Notice {
  const StateNotice(this.state);
  final DaemonState state;
}

class HistoryNotice extends Notice {
  const HistoryNotice(this.entries);

  /// 最新的在前。
  final List<LogEntry> entries;
}

class LogNotice extends Notice {
  const LogNotice(this.entry);
  final LogEntry entry;
}

class PairingNotice extends Notice {
  const PairingNotice(this.prompt);
  final PairingPrompt prompt;
}

class PairingClosedNotice extends Notice {
  const PairingClosedNotice(this.id);
  final int id;
}

class ToastNotice extends Notice {
  const ToastNotice(this.text);
  final String text;
}

// ── 界面 → 后台 ──────────────────────────────

sealed class Request {
  const Request();

  Json toJson();
}

class HelloRequest extends Request {
  const HelloRequest(this.token, {this.version = ipcVersion});
  final String token;
  final int version;

  @override
  Json toJson() => {'type': 'hello', 'token': token, 'version': version};
}

class SetSharingRequest extends Request {
  const SetSharingRequest(this.on);
  final bool on;

  @override
  Json toJson() => {'type': 'set_sharing', 'on': on};
}

class UpdateSettingsRequest extends Request {
  const UpdateSettingsRequest(this.settings);
  final SettingsPatch settings;

  @override
  Json toJson() => {'type': 'update_settings', 'settings': settings.toJson()};
}

class ForgetRequest extends Request {
  const ForgetRequest(this.fingerprint);
  final String fingerprint;

  @override
  Json toJson() => {'type': 'forget', 'fingerprint': fingerprint};
}

class PairReplyRequest extends Request {
  const PairReplyRequest(this.id, {required this.accept});
  final int id;
  final bool accept;

  @override
  Json toJson() => {'type': 'pair_reply', 'id': id, 'accept': accept};
}

class ConnectRequest extends Request {
  const ConnectRequest(this.fingerprint);
  final String fingerprint;

  @override
  Json toJson() => {'type': 'connect', 'fingerprint': fingerprint};
}

class OpenPermissionSettingsRequest extends Request {
  const OpenPermissionSettingsRequest();

  @override
  Json toJson() => {'type': 'open_permission_settings'};
}

class ClearLogRequest extends Request {
  const ClearLogRequest();

  @override
  Json toJson() => {'type': 'clear_log'};
}

class OpenSettingsRequest extends Request {
  const OpenSettingsRequest();

  @override
  Json toJson() => {'type': 'open_settings'};
}

/// 把这些文件和文件夹（本机绝对路径）发给连着的设备 [to]（证书指纹）；
/// 不指定时后台发给光标最近去过的那台。
class SendFilesRequest extends Request {
  const SendFilesRequest(this.paths, {this.to});
  final List<String> paths;
  final String? to;

  @override
  Json toJson() => {'type': 'send_files', 'paths': paths, 'to': ?to};
}

/// 要修改的设置；为 null 的字段不变，也不会发出去。
class SettingsPatch {
  const SettingsPatch({
    this.name,
    this.role,
    this.side,
    this.layout,
    this.swapCmdCtrl,
    this.announce,
    this.clipboard,
    this.server,
    this.autostart,
    this.theme,
    this.accent,
  });

  final String? name;

  /// 改变本机的角色。后台会断开现有的连接、以新角色重新开始。
  final Role? role;
  final ScreenSide? side;

  /// 给出时整份替换。
  final List<Placement>? layout;
  final bool? swapCmdCtrl;
  final bool? announce;
  final bool? clipboard;
  final String? server;
  final bool? autostart;
  final UiTheme? theme;
  final int? accent;

  Json toJson() => {
    'name': ?name,
    'role': ?role?.name,
    'side': ?side?.name,
    'layout': ?layout?.map((p) => p.toJson()).toList(),
    'swap_cmd_ctrl': ?swapCmdCtrl,
    'announce': ?announce,
    'clipboard': ?clipboard,
    'server': ?server,
    'autostart': ?autostart,
    'theme': ?theme?.name,
    'accent': ?accent,
  };

  /// 后面的补丁覆盖前面的。
  SettingsPatch merge(SettingsPatch o) => SettingsPatch(
    name: o.name ?? name,
    role: o.role ?? role,
    side: o.side ?? side,
    layout: o.layout ?? layout,
    swapCmdCtrl: o.swapCmdCtrl ?? swapCmdCtrl,
    announce: o.announce ?? announce,
    clipboard: o.clipboard ?? clipboard,
    server: o.server ?? server,
    autostart: o.autostart ?? autostart,
    theme: o.theme ?? theme,
    accent: o.accent ?? accent,
  );

  /// 去掉已经与 `s` 一致的字段：后台确认过的改动不再需要覆盖。
  SettingsPatch without(DaemonSettings s) => SettingsPatch(
    name: name == s.name ? null : name,
    // 角色不在设置里，界面直接看后台报告的 `local.role`，不需要先显示。
    role: null,
    side: side == s.side ? null : side,
    layout: switch (layout) {
      final l? when !_sameList(l, s.layout) => l,
      _ => null,
    },
    swapCmdCtrl: swapCmdCtrl == s.swapCmdCtrl ? null : swapCmdCtrl,
    announce: announce == s.announce ? null : announce,
    clipboard: clipboard == s.clipboard ? null : clipboard,
    server: server == s.server ? null : server,
    autostart: autostart == s.autostart ? null : autostart,
    theme: theme == s.theme ? null : theme,
    accent: accent == s.accent ? null : accent,
  );

  bool get isEmpty => toJson().isEmpty;
}
