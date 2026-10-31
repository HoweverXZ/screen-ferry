// 数据来源：界面只通过 Backend 拿状态、发命令。
// 运行时用 IpcBackend 连真实的后台；测试和看设计用 DemoBackend（见 demo_backend.dart）。

import 'package:flutter/foundation.dart';

import '../ipc/client.dart';
import '../ipc/messages.dart';

export '../ipc/client.dart' show DaemonLink;

abstract class Backend {
  /// 与后台的连接状态。
  ValueListenable<DaemonLink> get link;

  /// 后台推来的通知。
  Stream<Notice> get notices;

  void start();

  /// 发一条命令；没连上后台时返回 false。
  bool send(Request request);

  Future<void> dispose();
}

/// 经本机回环端口连接真实的后台。
class IpcBackend implements Backend {
  IpcBackend({String? path}) : _client = IpcClient(path: path);

  final IpcClient _client;

  @override
  ValueListenable<DaemonLink> get link => _client.link;

  @override
  Stream<Notice> get notices => _client.notices;

  @override
  void start() => _client.start();

  @override
  bool send(Request request) => _client.send(request);

  @override
  Future<void> dispose() => _client.dispose();
}
