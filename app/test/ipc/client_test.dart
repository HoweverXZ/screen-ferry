// IPC 客户端：用本机回环上的假后台验证握手、收通知、断线重连和发命令。

import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:screenferry_settings/ipc/client.dart';
import 'package:screenferry_settings/ipc/endpoint.dart';
import 'package:screenferry_settings/ipc/messages.dart';

/// 假后台：在回环端口上监听，把收到的每一行交给测试，测试决定回什么。
class FakeDaemon {
  FakeDaemon._(this._server, this.token);

  static Future<FakeDaemon> start({String token = 'secret-token'}) async {
    final server = await ServerSocket.bind(InternetAddress.loopbackIPv4, 0);
    final daemon = FakeDaemon._(server, token);
    server.listen(daemon._accept);
    return daemon;
  }

  final ServerSocket _server;
  final String token;
  final connections = StreamController<FakeConnection>();
  late final _queue = StreamIterator(connections.stream);

  int get port => _server.port;

  void _accept(Socket socket) => connections.add(FakeConnection(socket));

  Future<FakeConnection> next() async {
    expect(await _queue.moveNext().timeout(const Duration(seconds: 5)), isTrue);
    return _queue.current;
  }

  Future<void> writeEndpoint(String path) =>
      File(path).writeAsString(jsonEncode({'port': port, 'token': token, 'pid': 1, 'version': ipcVersion}));

  Future<void> close() async {
    // 先停掉读取：暂停中的 StreamIterator 收不到结束事件，直接 close 会一直等。
    await _queue.cancel();
    await _server.close();
  }
}

class FakeConnection {
  FakeConnection(this.socket) {
    _lines = StreamIterator(socket.cast<List<int>>().transform(utf8.decoder).transform(const LineSplitter()));
  }

  final Socket socket;
  late final StreamIterator<String> _lines;

  Future<Map<String, Object?>> read() async {
    expect(await _lines.moveNext().timeout(const Duration(seconds: 5)), isTrue);
    return jsonDecode(_lines.current) as Map<String, Object?>;
  }

  void write(Map<String, Object?> json) => socket.write('${jsonEncode(json)}\n');
}

/// 等到连接状态变成 `want`。
Future<void> until(IpcClient client, DaemonLink want) async {
  if (client.link.value == want) return;
  final done = Completer<void>();
  void check() {
    if (client.link.value == want && !done.isCompleted) done.complete();
  }

  client.link.addListener(check);
  try {
    await done.future.timeout(const Duration(seconds: 5));
  } finally {
    client.link.removeListener(check);
  }
}

const stateJson = {
  'type': 'state',
  'state': {
    'local': {
      'name': 'Mac Studio',
      'fingerprint': 'aa',
      'short': 'AA',
      'role': 'server',
      'data_dir': '/d',
      'version': '0.1.0',
    },
    'sharing': true,
    'problem': null,
    'link': {'status': 'waiting', 'peer': null, 'since': null, 'cursor_on_peer': false, 'rtt_ms': <double>[]},
    'clipboard': null,
    'peers': <Object>[],
    'found': <Object>[],
    'settings': {
      'name': '',
      'side': 'right',
      'announce': true,
      'clipboard': true,
      'server': '',
      'autostart': false,
      'theme': 'system',
      'accent': 0,
    },
    'activity': <Object>[],
  },
};

void main() {
  late Directory dir;
  late String path;

  setUp(() async {
    dir = await Directory.systemTemp.createTemp('sf-ipc-');
    path = '${dir.path}/ipc.json';
  });

  tearDown(() => dir.delete(recursive: true));

  test('没有 ipc.json 时视为后台没有运行', () async {
    final client = IpcClient(path: path, retry: const Duration(milliseconds: 50))..start();
    await until(client, DaemonLink.offline);
    expect(client.send(const ClearLogRequest()), isFalse, reason: '没连上时命令直接丢弃');
    await client.dispose();
  });

  test('带令牌握手，收到状态，断线后自动重连', () async {
    final daemon = await FakeDaemon.start();
    await daemon.writeEndpoint(path);
    final client = IpcClient(path: path, retry: const Duration(milliseconds: 50))..start();
    final notices = <Notice>[];
    final sub = client.notices.listen(notices.add);

    var conn = await daemon.next();
    final hello = await conn.read();
    expect(hello, {'type': 'hello', 'token': daemon.token, 'version': ipcVersion});
    expect(client.link.value, isNot(DaemonLink.online), reason: '收到 welcome 之前不算连上');

    conn.write({'type': 'welcome', 'version': ipcVersion});
    conn.write(stateJson);
    await until(client, DaemonLink.online);
    await Future<void>.delayed(const Duration(milliseconds: 50));
    final state = notices.whereType<StateNotice>().single.state;
    expect(state.local.name, 'Mac Studio');
    expect(state.link.status, LinkStatus.waiting);

    // 发命令：一行一条 JSON。
    expect(client.send(const SetSharingRequest(false)), isTrue);
    expect(await conn.read(), {'type': 'set_sharing', 'on': false});

    // 后台断开：变为未连接，随后自动重连并重新握手。
    await conn.socket.close();
    await until(client, DaemonLink.offline);
    conn = await daemon.next();
    expect((await conn.read())['type'], 'hello');
    conn.write({'type': 'welcome', 'version': ipcVersion});
    await until(client, DaemonLink.online);

    await sub.cancel();
    await client.dispose();
    await daemon.close();
  });

  test('协议版本不同就断开，不当作连上', () async {
    final daemon = await FakeDaemon.start();
    await daemon.writeEndpoint(path);
    final client = IpcClient(path: path, retry: const Duration(seconds: 10))..start();
    final conn = await daemon.next();
    await conn.read();
    conn.write({'type': 'welcome', 'version': ipcVersion + 1});
    await until(client, DaemonLink.offline);
    await client.dispose();
    await daemon.close();
  });

  test('ipc.json 的位置', () {
    expect(
      endpointPath(environment: {'HOME': '/Users/me'}, os: 'macos'),
      '/Users/me/Library/Caches/ScreenFerry/ipc.json',
    );
    expect(
      endpointPath(environment: {'LOCALAPPDATA': r'C:\Users\me\AppData\Local'}, os: 'windows'),
      r'C:\Users\me\AppData\Local\ScreenFerry\ipc.json',
    );
    expect(endpointPath(environment: {'SCREENFERRY_IPC': '/tmp/x.json', 'HOME': '/h'}, os: 'macos'), '/tmp/x.json');
    expect(Endpoint.parse('{"port": 1}'), isNull, reason: '缺令牌');
    expect(Endpoint.parse('not json'), isNull);
  });
}
