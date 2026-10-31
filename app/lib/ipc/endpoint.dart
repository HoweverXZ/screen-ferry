// 找到后台：读取后台写下的 ipc.json（端口与令牌），位置见 docs/ipc.md。

import 'dart:convert';
import 'dart:io';

class Endpoint {
  const Endpoint({required this.port, required this.token, required this.pid, required this.version});

  final int port;
  final String token;
  final int pid;
  final int version;

  /// 内容不完整时返回 null。
  static Endpoint? parse(String text) {
    try {
      final j = jsonDecode(text);
      if (j is! Map<String, Object?>) return null;
      final (port, token) = (j['port'], j['token']);
      if (port is! int || token is! String || token.isEmpty) return null;
      return Endpoint(
        port: port,
        token: token,
        pid: j['pid'] is int ? j['pid'] as int : 0,
        version: j['version'] is int ? j['version'] as int : 0,
      );
    } on FormatException {
      return null;
    }
  }
}

/// `ipc.json` 的位置。环境变量 `SCREENFERRY_IPC` 优先（调试 `--data-dir` 时用）。
String? endpointPath({Map<String, String>? environment, String? os}) {
  final env = environment ?? Platform.environment;
  final override = env['SCREENFERRY_IPC'];
  if (override != null && override.isNotEmpty) return override;
  switch (os ?? Platform.operatingSystem) {
    case 'windows':
      final local = env['LOCALAPPDATA'];
      return local == null || local.isEmpty ? null : '$local\\ScreenFerry\\ipc.json';
    case 'macos':
      final home = env['HOME'];
      return home == null || home.isEmpty ? null : '$home/Library/Caches/ScreenFerry/ipc.json';
    default:
      final cache = env['XDG_CACHE_HOME'];
      if (cache != null && cache.isNotEmpty) return '$cache/screenferry/ipc.json';
      final home = env['HOME'];
      return home == null || home.isEmpty ? null : '$home/.cache/screenferry/ipc.json';
  }
}

/// 读取连接信息；文件不存在或内容不对时为 null，表示后台没有运行。
Future<Endpoint?> readEndpoint(String? path) async {
  if (path == null) return null;
  try {
    return Endpoint.parse(await File(path).readAsString());
  } on FileSystemException {
    return null;
  }
}
