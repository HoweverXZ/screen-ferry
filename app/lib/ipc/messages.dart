// 本机通信协议的消息类型，与 crates/ferry-ipc/src/message.rs 一一对应，说明见 docs/ipc.md。
// 两边靠 test/fixtures/ipc_samples.json 中的样例保持一致；改协议时两边一起改。
// 不认识的字段忽略，不认识的取值退回到合理的缺省值。

/// 协议版本。
const ipcVersion = 2;

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

/// Dart 枚举名转成 snake_case 的取值（`capsLock` → `caps_lock`）。
String _snake(Enum v) => v.name.replaceAllMapped(RegExp('[A-Z]'), (m) => '_${m[0]!.toLowerCase()}');

// ── 枚举 ─────────────────────────────────────

enum Role { server, client }

enum ScreenSide { left, right, top, bottom }

/// 设置界面的配色，取值与 Flutter 的 ThemeMode 同名。
enum UiTheme { system, light, dark }

/// 界面语言：跟随系统、简体中文、英文。
enum UiLanguage { system, zh, en }

enum LinkStatus { paused, starting, waiting, searching, connecting, connected, stopped }

enum ProblemKind { permission, portInUse, other }

enum LogKind { cursor, clip, link, file }

enum LogIcon { power, link, unlink, lock, cursor, clipboard, pause, play, layout, radar, warning, settings, file }

/// 修饰键方案：本机习惯（两台键盘不同时互换 ⌘ 与 Ctrl）、原样、自定义。
enum ModifierPreset { habit, plain, custom }

/// 修饰键要变成的键。`gui` 是 Mac 的 ⌘、Windows 的 Windows 键；`none` 是不发送。
enum ModifierTarget { ctrl, shift, alt, gui, capsLock, none }

/// 四个修饰键，按 USB HID 区分。
enum ModifierKey { gui, ctrl, alt, capsLock }

enum ScrollSpeed { slow, normal, fast }

/// 按地址连接、邀请为什么没成。
enum DialError { badAddress, refused, unreachable, notScreenFerry, incompatible, busy, wrongRole }

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
    this.naturalScrolling,
    this.addresses = const [],
    this.invitePort,
    this.system = '',
    this.verboseSince,
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
    naturalScrolling: j['natural_scrolling'] is bool ? j['natural_scrolling'] as bool : null,
    addresses: [
      for (final a in (j['addresses'] is List ? j['addresses'] as List : const []))
        if (a is String) a,
    ],
    invitePort: j['invite_port'] is num ? _int(j['invite_port']) : null,
    system: _str(j['system']),
    verboseSince: j['verbose_since'] is num ? DateTime.fromMillisecondsSinceEpoch(_int(j['verbose_since'])) : null,
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

  /// macOS：系统设置里开着“自然滚动”。其他系统为 null。
  final bool? naturalScrolling;

  /// 本机的局域网地址，常见网段在前。另一台电脑按地址连接时填它。
  final List<String> addresses;

  /// 客户端：等待邀请的端口；服务端为 null。
  final int? invitePort;

  /// 本机系统和版本，例如“macOS 15.6 · arm64”。旧版后台不报，为空。
  final String system;

  /// 详细记录从什么时候开着；关着时为 null。
  final DateTime? verboseSince;
}

class Problem {
  const Problem({required this.kind, required this.message, this.port});

  factory Problem.fromJson(Json j) => Problem(
    kind: _enum(ProblemKind.values, _camel(j['kind']), ProblemKind.other),
    message: _str(j['message']),
    port: j['port'] is num ? _int(j['port']) : null,
  );

  final ProblemKind kind;

  /// 后台给的中文说明。[ProblemKind.permission]、[ProblemKind.portInUse] 由界面按当前语言说。
  final String message;

  /// [ProblemKind.portInUse]：被占用的端口。
  final int? port;
}

/// 一句给用户看的话：界面按 [key] 选当前语言的说法，把 [args] 填进去。
class Phrase {
  const Phrase(this.key, [this.args = const {}]);

  static Phrase? fromJson(Object? v) {
    if (v is! Json) return null;
    final args = v['args'];
    return Phrase(_str(v['key']), {
      if (args is Json)
        for (final e in args.entries)
          if (e.value is String) e.key: e.value as String,
    });
  }

  final String key;
  final Map<String, String> args;

  String arg(String name) => args[name] ?? '';
  int number(String name) => int.tryParse(args[name] ?? '') ?? 0;
  bool flag(String name) => args[name] == 'true';
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
    this.rich = false,
    this.image,
    required this.at,
  });

  factory ClipboardSync.fromJson(Json j) => ClipboardSync(
    toPeer: _bool(j['to_peer']),
    peer: _str(j['peer']),
    chars: j['chars'] is num ? _int(j['chars']) : null,
    bytes: _int(j['bytes']),
    preview: _str(j['preview']),
    rich: _bool(j['rich']),
    image: j['image'] is Json ? ClipboardImage.fromJson(j['image'] as Json) : null,
    at: _time(j['at']) ?? DateTime.fromMillisecondsSinceEpoch(0),
  );

  final bool toPeer;
  final String peer;

  /// 文字的字符数；没有文字时为 null。
  final int? chars;
  final int bytes;
  final String preview;

  /// 带格式（HTML 或 RTF）。
  final bool rich;

  /// 有图片时的尺寸和预览。
  final ClipboardImage? image;
  final DateTime at;
}

/// 剪贴板里的一张图片。
class ClipboardImage {
  const ClipboardImage({required this.width, required this.height, this.path});

  factory ClipboardImage.fromJson(Json j) => ClipboardImage(
    width: _int(j['width']),
    height: _int(j['height']),
    path: j['path'] is String ? j['path'] as String : null,
  );

  final int width;
  final int height;

  /// 预览用的 PNG 文件；后台没能存下来时为 null。
  final String? path;
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
    this.alias = '',
    this.platform = '',
    this.addr = '',
  });

  factory PeerInfo.fromJson(Json j) => PeerInfo(
    name: _str(j['name']),
    fingerprint: _str(j['fingerprint']),
    short: _str(j['short']),
    online: _bool(j['online']),
    lastSeen: _time(j['last_seen']),
    side: _enumOrNull(ScreenSide.values, j['side']),
    cursor: _bool(j['cursor']),
    alias: _str(j['alias']),
    platform: _str(j['platform']),
    addr: _str(j['addr']),
  );

  /// 它自己报的名字。
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

  /// 本机给它起的名字；空为没起。
  final String alias;

  /// 它是什么系统：`macos`、`windows`；本次运行还没连上过、不知道时为空。
  final String platform;

  /// 上次连上时它的地址（只知道 IP 时就是 IP）；没连上过为空。
  final String addr;

  /// 显示给用户的名字：起过名字就用起的。
  String get displayName => alias.trim().isEmpty ? name : alias.trim();
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
    required this.announce,
    required this.clipboard,
    this.clipboardLimitMb = 32,
    this.mouseDpi = 1600,
    required this.server,
    required this.autostart,
    required this.theme,
    required this.accent,
    this.language = UiLanguage.system,
    this.devices = const [],
  });

  /// 剪贴板单项大小上限的取值范围（MB）。
  static const clipboardLimitRange = (min: 1, max: 64);

  /// 本机鼠标 DPI 的取值范围，与每台设备的光标速度相同。
  static const mouseDpiRange = DeviceSettings.pointerDpiRange;

  factory DaemonSettings.fromJson(Json j) => DaemonSettings(
    name: _str(j['name']),
    side: _enum(ScreenSide.values, j['side'], ScreenSide.right),
    layout: [for (final p in _list(j['layout'])) Placement.fromJson(p)],
    announce: _bool(j['announce']),
    clipboard: _bool(j['clipboard']),
    clipboardLimitMb: j['clipboard_limit_mb'] is num ? _int(j['clipboard_limit_mb']) : 32,
    mouseDpi: j['mouse_dpi'] is num ? _int(j['mouse_dpi']) : 1600,
    server: _str(j['server']),
    autostart: _bool(j['autostart']),
    theme: _enum(UiTheme.values, j['theme'], UiTheme.system),
    accent: _int(j['accent']),
    language: _enum(UiLanguage.values, j['language'], UiLanguage.system),
    devices: [for (final d in _list(j['devices'])) DeviceSettings.fromJson(d)],
  );

  final String name;

  /// 没有在 [layout] 里单独安排的设备放在本机哪一侧，排在已安排的后面。
  final ScreenSide side;

  /// 单独安排过位置的设备，按先后排列：同一侧的几台按这个先后平分那条边
  /// （左右两侧从上到下，上下两侧从左到右）。
  final List<Placement> layout;
  final bool announce;
  final bool clipboard;

  /// 剪贴板单项（一种格式）的大小上限，单位 MB。
  final int clipboardLimitMb;

  /// 本机鼠标的 DPI。每台设备的光标速度以它为准。
  final int mouseDpi;
  final String server;
  final bool autostart;
  final UiTheme theme;
  final int accent;

  /// 界面语言。
  final UiLanguage language;

  /// 单独设置过的设备；没列出的用缺省设置。
  final List<DeviceSettings> devices;

  /// 一台设备的设置；没有单独设置过的用缺省设置。
  DeviceSettings device(String fingerprint) =>
      devices.where((d) => d.fingerprint == fingerprint).firstOrNull ?? DeviceSettings(fingerprint: fingerprint);

  /// 叠加一份补丁，界面在后台确认之前先按它显示。
  DaemonSettings apply(SettingsPatch p) => DaemonSettings(
    name: p.name ?? name,
    side: p.side ?? side,
    layout: p.layout ?? layout,
    announce: p.announce ?? announce,
    clipboard: p.clipboard ?? clipboard,
    clipboardLimitMb: p.clipboardLimitMb ?? clipboardLimitMb,
    mouseDpi: p.mouseDpi ?? mouseDpi,
    server: p.server ?? server,
    autostart: p.autostart ?? autostart,
    theme: p.theme ?? theme,
    accent: p.accent ?? accent,
    language: p.language ?? language,
    devices: devices,
  );

  /// 换上一台设备的设置（界面在后台确认之前先按它显示）。全是缺省值时去掉这一条。
  DaemonSettings withDevice(DeviceSettings d) {
    final rest = [for (final x in devices) x.fingerprint == d.fingerprint ? d : x];
    final next = rest.any((x) => x.fingerprint == d.fingerprint) ? rest : [...rest, d];
    return DaemonSettings(
      name: name,
      side: side,
      layout: layout,
      announce: announce,
      clipboard: clipboard,
      clipboardLimitMb: clipboardLimitMb,
      mouseDpi: mouseDpi,
      server: server,
      autostart: autostart,
      theme: theme,
      accent: accent,
      language: language,
      devices: [
        for (final x in next)
          if (!x.isDefault) x,
      ],
    );
  }
}

/// 把一个键换成另一个键。键码是 USB HID 扩展用法码：高 16 位用法页，低 16 位用法 ID。
class KeyMapping {
  const KeyMapping(this.from, this.to);

  factory KeyMapping.fromJson(Json j) => KeyMapping(_int(j['from']), _int(j['to']));

  final int from;

  /// 0 表示不发送。
  final int to;

  Json toJson() => {'from': from, 'to': to};

  @override
  bool operator ==(Object other) => other is KeyMapping && other.from == from && other.to == to;

  @override
  int get hashCode => Object.hash(from, to);
}

/// 四个修饰键各自变成什么。
class ModifierMap {
  const ModifierMap({
    this.gui = ModifierTarget.gui,
    this.ctrl = ModifierTarget.ctrl,
    this.alt = ModifierTarget.alt,
    this.capsLock = ModifierTarget.capsLock,
  });

  factory ModifierMap.fromJson(Json j) => ModifierMap(
    gui: _enum(ModifierTarget.values, _camel(j['gui']), ModifierTarget.gui),
    ctrl: _enum(ModifierTarget.values, _camel(j['ctrl']), ModifierTarget.ctrl),
    alt: _enum(ModifierTarget.values, _camel(j['alt']), ModifierTarget.alt),
    capsLock: _enum(ModifierTarget.values, _camel(j['caps_lock']), ModifierTarget.capsLock),
  );

  /// 原样：每个键还是它自己。
  static const plain = ModifierMap();

  /// 互换 ⌘（Windows 键）与 Ctrl。
  static const swapGuiCtrl = ModifierMap(gui: ModifierTarget.ctrl, ctrl: ModifierTarget.gui);

  final ModifierTarget gui;
  final ModifierTarget ctrl;
  final ModifierTarget alt;
  final ModifierTarget capsLock;

  ModifierTarget operator [](ModifierKey key) => switch (key) {
    ModifierKey.gui => gui,
    ModifierKey.ctrl => ctrl,
    ModifierKey.alt => alt,
    ModifierKey.capsLock => capsLock,
  };

  /// 把 [key] 换成 [to]，其余不变。
  ModifierMap replaced(ModifierKey key, ModifierTarget to) => ModifierMap(
    gui: key == ModifierKey.gui ? to : gui,
    ctrl: key == ModifierKey.ctrl ? to : ctrl,
    alt: key == ModifierKey.alt ? to : alt,
    capsLock: key == ModifierKey.capsLock ? to : capsLock,
  );

  Json toJson() => {'gui': _snake(gui), 'ctrl': _snake(ctrl), 'alt': _snake(alt), 'caps_lock': _snake(capsLock)};

  @override
  bool operator ==(Object other) =>
      other is ModifierMap && other.gui == gui && other.ctrl == ctrl && other.alt == alt && other.capsLock == capsLock;

  @override
  int get hashCode => Object.hash(gui, ctrl, alt, capsLock);
}

/// 一台设备的单独设置，与 `crates/ferry-types/src/device.rs` 一致。
class DeviceSettings {
  const DeviceSettings({
    required this.fingerprint,
    this.alias = '',
    this.pointerDpi = 0,
    this.reverseVertical = false,
    this.reverseHorizontal = false,
    this.scrollSpeed = ScrollSpeed.normal,
    this.modifiers = ModifierPreset.habit,
    this.textShortcuts = false,
    this.edgeDelayMs = 0,
    this.clipboard = true,
    this.files = true,
    this.customModifiers = ModifierMap.plain,
    this.keys = const [],
  });

  factory DeviceSettings.fromJson(Json j) => DeviceSettings(
    fingerprint: _str(j['fingerprint']),
    alias: _str(j['alias']),
    pointerDpi: _int(j['pointer_dpi']),
    reverseVertical: _bool(j['reverse_vertical']),
    reverseHorizontal: _bool(j['reverse_horizontal']),
    scrollSpeed: _enum(ScrollSpeed.values, j['scroll_speed'], ScrollSpeed.normal),
    modifiers: _enum(ModifierPreset.values, j['modifiers'], ModifierPreset.habit),
    textShortcuts: _bool(j['text_shortcuts']),
    edgeDelayMs: _int(j['edge_delay_ms']),
    clipboard: j['clipboard'] is bool ? j['clipboard'] as bool : true,
    files: j['files'] is bool ? j['files'] as bool : true,
    customModifiers: j['custom_modifiers'] is Json
        ? ModifierMap.fromJson(j['custom_modifiers'] as Json)
        : ModifierMap.plain,
    keys: [for (final k in _list(j['keys'])) KeyMapping.fromJson(k)],
  );

  /// 光标速度的取值范围（DPI）。
  static const pointerDpiRange = (min: 100, max: 12800);

  /// 设备的证书指纹，64 位十六进制。
  final String fingerprint;

  /// 本机给它起的名字，只在本机显示；空为它自己报的名字。
  final String alias;

  /// 光标在它上面时就像接着一个多少 DPI 的鼠标；0 表示和本机一样。
  final int pointerDpi;
  final bool reverseVertical;
  final bool reverseHorizontal;
  final ScrollSpeed scrollSpeed;
  final ModifierPreset modifiers;

  /// 两台不是同一种键盘时，文本编辑快捷键按本机的习惯转换。
  final bool textShortcuts;

  /// 光标要在边上停这么久（毫秒）才切过去。
  final int edgeDelayMs;
  final bool clipboard;
  final bool files;

  /// [modifiers] 为自定义时四个修饰键各自变成什么。
  final ModifierMap customModifiers;

  /// 其他按键映射。
  final List<KeyMapping> keys;

  /// 除了指纹都是缺省值。
  bool get isDefault => this == DeviceSettings(fingerprint: fingerprint);

  /// 实际的修饰键对应关系。[differ]：两台不是同一种键盘（一台 Mac、一台不是）。
  ModifierMap modifierMap({required bool differ}) => switch (modifiers) {
    ModifierPreset.habit => differ ? ModifierMap.swapGuiCtrl : ModifierMap.plain,
    ModifierPreset.plain => ModifierMap.plain,
    ModifierPreset.custom => customModifiers,
  };

  /// 恢复默认：只留下名字。
  DeviceSettings reset() => DeviceSettings(fingerprint: fingerprint, alias: alias);

  DeviceSettings copyWith({
    String? alias,
    int? pointerDpi,
    bool? reverseVertical,
    bool? reverseHorizontal,
    ScrollSpeed? scrollSpeed,
    ModifierPreset? modifiers,
    bool? textShortcuts,
    int? edgeDelayMs,
    bool? clipboard,
    bool? files,
    ModifierMap? customModifiers,
    List<KeyMapping>? keys,
  }) => DeviceSettings(
    fingerprint: fingerprint,
    alias: alias ?? this.alias,
    pointerDpi: pointerDpi ?? this.pointerDpi,
    reverseVertical: reverseVertical ?? this.reverseVertical,
    reverseHorizontal: reverseHorizontal ?? this.reverseHorizontal,
    scrollSpeed: scrollSpeed ?? this.scrollSpeed,
    modifiers: modifiers ?? this.modifiers,
    textShortcuts: textShortcuts ?? this.textShortcuts,
    edgeDelayMs: edgeDelayMs ?? this.edgeDelayMs,
    clipboard: clipboard ?? this.clipboard,
    files: files ?? this.files,
    customModifiers: customModifiers ?? this.customModifiers,
    keys: keys ?? this.keys,
  );

  Json toJson() => {
    'fingerprint': fingerprint,
    'alias': alias,
    'pointer_dpi': pointerDpi,
    'reverse_vertical': reverseVertical,
    'reverse_horizontal': reverseHorizontal,
    'scroll_speed': scrollSpeed.name,
    'modifiers': modifiers.name,
    'text_shortcuts': textShortcuts,
    'edge_delay_ms': edgeDelayMs,
    'clipboard': clipboard,
    'files': files,
    'custom_modifiers': customModifiers.toJson(),
    'keys': [for (final k in keys) k.toJson()],
  };

  @override
  bool operator ==(Object other) =>
      other is DeviceSettings &&
      other.fingerprint == fingerprint &&
      other.alias == alias &&
      other.pointerDpi == pointerDpi &&
      other.reverseVertical == reverseVertical &&
      other.reverseHorizontal == reverseHorizontal &&
      other.scrollSpeed == scrollSpeed &&
      other.modifiers == modifiers &&
      other.textShortcuts == textShortcuts &&
      other.edgeDelayMs == edgeDelayMs &&
      other.clipboard == clipboard &&
      other.files == files &&
      other.customModifiers == customModifiers &&
      _sameList(other.keys, keys);

  @override
  int get hashCode => Object.hash(
    fingerprint,
    alias,
    pointerDpi,
    reverseVertical,
    reverseHorizontal,
    scrollSpeed,
    modifiers,
    textShortcuts,
    edgeDelayMs,
    clipboard,
    files,
    customModifiers,
    Object.hashAll(keys),
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
    this.phrase,
  });

  factory LogEntry.fromJson(Json j) => LogEntry(
    id: _int(j['id']),
    kind: _enum(LogKind.values, j['kind'], LogKind.link),
    icon: _enum(LogIcon.values, j['icon'], LogIcon.settings),
    text: _str(j['text']),
    detail: _str(j['detail']),
    at: _time(j['at']) ?? DateTime.fromMillisecondsSinceEpoch(0),
    phrase: Phrase.fromJson(j['phrase']),
  );

  final int id;
  final LogKind kind;
  final LogIcon icon;

  /// 后台给的中文两行；认识 [phrase] 时按当前语言说，不认识时显示它们。
  final String text;
  final String detail;
  final DateTime at;
  final Phrase? phrase;
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
    'toast' => ToastNotice(_str(j['text']), phrase: Phrase.fromJson(j['phrase'])),
    'show_device' => ShowDeviceNotice(_str(j['fingerprint'])),
    'dialed' => DialedNotice(
      address: _str(j['address']),
      invite: _bool(j['invite']),
      error: _enumOrNull(DialError.values, _camel(j['error'])),
    ),
    'quit' => const QuitNotice(),
    'run_log' => RunLogNotice(
      lines: [for (final l in _list(j['lines'])) RunLogLine.fromJson(l)],
      next: _int(j['next']),
      fresh: _bool(j['fresh']),
      bytes: _int(j['bytes']),
      linesTotal: _int(j['lines_total']),
    ),
    'diagnostics_exported' => DiagnosticsExportedNotice(path: _str(j['path']), bytes: _int(j['bytes'])),
    'diagnostics_summary' => DiagnosticsSummaryNotice(_str(j['text'])),
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
  const ToastNotice(this.text, {this.phrase});

  /// 后台给的中文；认识 [phrase] 时按当前语言说。
  final String text;
  final Phrase? phrase;
}

/// 翻到这台设备的设置页：它请本机打开了设置。
class ShowDeviceNotice extends Notice {
  const ShowDeviceNotice(this.fingerprint);
  final String fingerprint;
}

/// 按地址连接（[invite] 为假）或邀请的结果。[error] 为 null 表示成了：
/// 连上了、开始核对配对码，或者对方接受了邀请、正在连过来。
class DialedNotice extends Notice {
  const DialedNotice({required this.address, required this.invite, this.error});
  final String address;
  final bool invite;
  final DialError? error;
}

/// 后台要退出了（托盘里的“退出”或 `quit` 命令），界面随之关闭。
class QuitNotice extends Notice {
  const QuitNotice();
}

/// 运行日志的一段。[fresh] 为真时整个替换（第一次读，或者日志换了一卷），否则接在后面。
/// [next] 是下次读的位置；[bytes]、[linesTotal] 是当前这卷的大小和行数。
class RunLogNotice extends Notice {
  const RunLogNotice({
    required this.lines,
    required this.next,
    required this.fresh,
    required this.bytes,
    required this.linesTotal,
  });
  final List<RunLogLine> lines;
  final int next;
  final bool fresh;
  final int bytes;
  final int linesTotal;
}

/// 诊断文件存好了：完整路径和大小。
class DiagnosticsExportedNotice extends Notice {
  const DiagnosticsExportedNotice({required this.path, required this.bytes});
  final String path;
  final int bytes;
}

/// 诊断摘要，复制到剪贴板。
class DiagnosticsSummaryNotice extends Notice {
  const DiagnosticsSummaryNotice(this.text);
  final String text;
}

enum RunLogLevel { debug, info, warn, error }

/// 运行日志的一行。
class RunLogLine {
  const RunLogLine({required this.at, required this.level, required this.text});

  factory RunLogLine.fromJson(Json j) => RunLogLine(
    at: _str(j['at']),
    level: _enum(RunLogLevel.values, j['level'], RunLogLevel.info),
    text: _str(j['text']),
  );

  /// 本机时间 `HH:MM:SS`；旧格式的行没有时间，为空。
  final String at;
  final RunLogLevel level;
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

/// 换上一台设备的单独设置，整份替换。
class SetDeviceRequest extends Request {
  const SetDeviceRequest(this.device);
  final DeviceSettings device;

  @override
  Json toJson() => {'type': 'set_device', 'device': device.toJson()};
}

/// 客户端：请连着的服务端 [fingerprint] 打开它的设置界面，翻到本机的设置页。
class OpenPeerSettingsRequest extends Request {
  const OpenPeerSettingsRequest(this.fingerprint);
  final String fingerprint;

  @override
  Json toJson() => {'type': 'open_peer_settings', 'fingerprint': fingerprint};
}

/// 客户端：按地址连接一台服务端（IP、IP:端口、电脑名），不写端口用 24810。
class ConnectAddressRequest extends Request {
  const ConnectAddressRequest(this.address);
  final String address;

  @override
  Json toJson() => {'type': 'connect_address', 'address': address};
}

/// 服务端：邀请这个地址上的电脑（被控制的一方）连过来，不写端口用 24811。
class InviteRequest extends Request {
  const InviteRequest(this.address);
  final String address;

  @override
  Json toJson() => {'type': 'invite', 'address': address};
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

/// 请后台退出：停下共享，通知界面一起关闭，然后退出。
class QuitRequest extends Request {
  const QuitRequest();

  @override
  Json toJson() => {'type': 'quit'};
}

/// 读运行日志：[since] 为空时取最近的一段，否则取上次 [RunLogNotice.next] 之后新写的。
class ReadRunLogRequest extends Request {
  const ReadRunLogRequest({this.since});
  final int? since;

  @override
  Json toJson() => {'type': 'read_run_log', 'since': ?since};
}

/// 把诊断文件存到“下载”文件夹。
class ExportDiagnosticsRequest extends Request {
  const ExportDiagnosticsRequest();

  @override
  Json toJson() => {'type': 'export_diagnostics'};
}

/// 取诊断摘要（版本、系统和最近 200 行运行日志）。
class CopyDiagnosticsRequest extends Request {
  const CopyDiagnosticsRequest();

  @override
  Json toJson() => {'type': 'copy_diagnostics'};
}

/// 在访达或资源管理器里显示最近导出的诊断文件。
class RevealDiagnosticsRequest extends Request {
  const RevealDiagnosticsRequest();

  @override
  Json toJson() => {'type': 'reveal_diagnostics'};
}

/// 打开运行日志所在的文件夹。
class OpenLogsFolderRequest extends Request {
  const OpenLogsFolderRequest();

  @override
  Json toJson() => {'type': 'open_logs_folder'};
}

/// 打开或关闭详细记录。
class SetVerboseRequest extends Request {
  const SetVerboseRequest(this.on);
  final bool on;

  @override
  Json toJson() => {'type': 'set_verbose', 'on': on};
}

/// 要修改的设置；为 null 的字段不变，也不会发出去。
class SettingsPatch {
  const SettingsPatch({
    this.name,
    this.role,
    this.side,
    this.layout,
    this.announce,
    this.clipboard,
    this.clipboardLimitMb,
    this.mouseDpi,
    this.server,
    this.autostart,
    this.theme,
    this.accent,
    this.language,
  });

  final String? name;

  /// 改变本机的角色。后台会断开现有的连接、以新角色重新开始。
  final Role? role;
  final ScreenSide? side;

  /// 给出时整份替换。
  final List<Placement>? layout;
  final bool? announce;
  final bool? clipboard;
  final int? clipboardLimitMb;
  final int? mouseDpi;
  final String? server;
  final bool? autostart;
  final UiTheme? theme;
  final int? accent;
  final UiLanguage? language;

  Json toJson() => {
    'name': ?name,
    'role': ?role?.name,
    'side': ?side?.name,
    'layout': ?layout?.map((p) => p.toJson()).toList(),
    'announce': ?announce,
    'clipboard': ?clipboard,
    'clipboard_limit_mb': ?clipboardLimitMb,
    'mouse_dpi': ?mouseDpi,
    'server': ?server,
    'autostart': ?autostart,
    'theme': ?theme?.name,
    'accent': ?accent,
    'language': ?language?.name,
  };

  /// 后面的补丁覆盖前面的。
  SettingsPatch merge(SettingsPatch o) => SettingsPatch(
    name: o.name ?? name,
    role: o.role ?? role,
    side: o.side ?? side,
    layout: o.layout ?? layout,
    announce: o.announce ?? announce,
    clipboard: o.clipboard ?? clipboard,
    clipboardLimitMb: o.clipboardLimitMb ?? clipboardLimitMb,
    mouseDpi: o.mouseDpi ?? mouseDpi,
    server: o.server ?? server,
    autostart: o.autostart ?? autostart,
    theme: o.theme ?? theme,
    accent: o.accent ?? accent,
    language: o.language ?? language,
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
    announce: announce == s.announce ? null : announce,
    clipboard: clipboard == s.clipboard ? null : clipboard,
    clipboardLimitMb: clipboardLimitMb == s.clipboardLimitMb ? null : clipboardLimitMb,
    mouseDpi: mouseDpi == s.mouseDpi ? null : mouseDpi,
    server: server == s.server ? null : server,
    autostart: autostart == s.autostart ? null : autostart,
    theme: theme == s.theme ? null : theme,
    accent: accent == s.accent ? null : accent,
    language: language == s.language ? null : language,
  );

  bool get isEmpty => toJson().isEmpty;
}
