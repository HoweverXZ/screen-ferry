// 本机通信客户端：连上后台的回环端口，发 hello，之后逐行收发 JSON。
// 连不上或断开时每秒重试一次，每次都重新读 ipc.json（后台每次启动都换端口和令牌）。

import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart';

import 'endpoint.dart';
import 'messages.dart';

/// 与后台的连接状态。
enum DaemonLink {
  /// 还没有得出结果（刚启动）。
  connecting,

  /// 后台没有运行，或连接断开了，正在重试。
  offline,

  /// 握手成功。
  online,
}

class IpcClient {
  IpcClient({String? path, this.retry = const Duration(seconds: 1)}) : _path = path ?? endpointPath();

  final String? _path;
  final Duration retry;

  final link = ValueNotifier(DaemonLink.connecting);
  final _notices = StreamController<Notice>.broadcast();
  Stream<Notice> get notices => _notices.stream;

  Socket? _socket;
  StreamSubscription<String>? _lines;
  Timer? _retry;
  bool _running = false;
  bool _attempting = false;

  void start() {
    if (_running) return;
    _running = true;
    _attempt();
  }

  /// 发一条命令。没连上时丢弃并返回 false。
  bool send(Request request) {
    final socket = _socket;
    if (socket == null || link.value != DaemonLink.online) return false;
    socket.write('${jsonEncode(request.toJson())}\n');
    return true;
  }

  Future<void> dispose() async {
    _running = false;
    _retry?.cancel();
    await _drop();
    link.dispose();
    await _notices.close();
  }

  Future<void> _attempt() async {
    if (!_running || _attempting) return;
    _attempting = true;
    try {
      final endpoint = await readEndpoint(_path);
      if (endpoint == null) return _failed();
      final Socket socket;
      try {
        socket = await Socket.connect(InternetAddress.loopbackIPv4, endpoint.port, timeout: const Duration(seconds: 2));
      } on SocketException {
        return _failed();
      }
      if (!_running) {
        socket.destroy();
        return;
      }
      socket.setOption(SocketOption.tcpNoDelay, true);
      _socket = socket;
      var welcomed = false;
      _lines = socket
          .cast<List<int>>()
          .transform(utf8.decoder)
          .transform(const LineSplitter())
          .listen(
            (line) {
              final notice = _decode(line);
              if (notice == null) return;
              if (!welcomed) {
                if (notice is WelcomeNotice && notice.version == ipcVersion) {
                  welcomed = true;
                  link.value = DaemonLink.online;
                } else {
                  _closed();
                }
                return;
              }
              _notices.add(notice);
            },
            onError: (Object _) => _closed(),
            onDone: _closed,
            cancelOnError: true,
          );
      socket.write('${jsonEncode(HelloRequest(endpoint.token).toJson())}\n');
    } finally {
      _attempting = false;
    }
  }

  Notice? _decode(String line) {
    try {
      final j = jsonDecode(line);
      return j is Map<String, Object?> ? Notice.fromJson(j) : null;
    } on FormatException {
      return null;
    }
  }

  void _failed() {
    if (!_running) return;
    link.value = DaemonLink.offline;
    _retry?.cancel();
    _retry = Timer(retry, _attempt);
  }

  void _closed() {
    unawaited(_drop());
    _failed();
  }

  Future<void> _drop() async {
    final lines = _lines;
    final socket = _socket;
    _lines = null;
    _socket = null;
    await lines?.cancel();
    socket?.destroy();
  }
}
