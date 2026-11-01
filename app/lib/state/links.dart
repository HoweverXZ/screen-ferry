// 用系统默认浏览器打开链接。

import 'dart:io';

/// 打开链接的方式。测试里换成只记下来，不真的打开浏览器。
typedef LinkOpener = Future<bool> Function(String url);

/// 用系统默认浏览器打开 [url]，发出了打开命令时返回 true。
Future<bool> openInBrowser(String url) async {
  try {
    if (Platform.isMacOS) return (await Process.run('open', [url])).exitCode == 0;
    if (Platform.isWindows) {
      // 交给系统的网址处理程序；不经过 cmd，不会闪出控制台窗口。
      await Process.start('rundll32', ['url.dll,FileProtocolHandler', url], mode: ProcessStartMode.detached);
      return true;
    }
    if (Platform.isLinux) return (await Process.run('xdg-open', [url])).exitCode == 0;
  } on ProcessException {
    return false;
  }
  return false;
}
