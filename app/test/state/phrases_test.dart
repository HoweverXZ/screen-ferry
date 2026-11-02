import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:screenferry_settings/ipc/messages.dart';
import 'package:screenferry_settings/l10n/l10n.dart';
import 'package:screenferry_settings/state/phrases.dart';

void main() {
  final zh = lookupAppLocalizations(const Locale('zh'));
  final en = lookupAppLocalizations(const Locale('en'));
  (String, String)? line(AppLocalizations l, String key, [Map<String, String> args = const {}, bool mac = true]) =>
      logLine(l, Phrase(key, args), mac: mac);

  const files = {'name': '办公室 PC', 'file': 'a.txt', 'items': '3', 'files': '5', 'bytes': '2048'};

  test('中文和后台附带的原文一样', () {
    expect(line(zh, 'cursor_to_peer', {'name': '办公室 PC', 'side': 'right'}), ('光标切到 办公室 PC', '从本机右侧边缘'));
    expect(line(zh, 'transfer_sending', files), ('发送 a.txt 等 3 项 到 办公室 PC', '5 个文件 · 2.0 KB'));
    expect(line(zh, 'listening', {'port': '24810', 'announced': 'true'}), ('已开始共享', '等待另一台电脑连接 · 端口 24810'));
    expect(toastText(zh, const Phrase('save_failed', {'message': '磁盘已满'})), '保存设置失败：磁盘已满');
  });

  test('英文按参数说', () {
    expect(line(en, 'transfer_sending', files), ('Sending a.txt and 2 more to 办公室 PC', '5 files · 2.0 KB'));
    expect(line(en, 'transfer_received', {...files, 'items': '1', 'files': '1'}), ('Received a.txt', ''));
    expect(
      line(en, 'clipboard_synced', {
        'name': 'PC',
        'to_peer': 'false',
        'content': 'image',
        'width': '800',
        'height': '600',
        'bytes': '1048576',
      }),
      ('Clipboard synced here', 'Image · 800×600 · 1.0 MB'),
    );
    expect(
      line(en, 'clipboard_skipped', {
        'name': 'PC',
        'to_peer': 'true',
        'format': 'text',
        'bytes': '2048',
        'limit': '1024',
      }),
      (
        'The text on the clipboard is too large to sync to PC',
        '2.0 KB, over the 1.0 KB limit · You can raise it in Settings',
      ),
    );
    expect(line(en, 'layout_moved', {'name': '', 'side': 'top'}), (
      'Other devices moved above this computer',
      'Layout saved',
    ));
    expect(
      line(en, 'cannot_share', {'problem': 'port_in_use', 'port': '24810', 'message': '端口被占用'})!.$2,
      contains('24810'),
    );
    expect(line(en, 'cursor_back', {'hotkey': 'true'}, false), ('Cursor back here', 'Shortcut Ctrl + Alt + Esc'));
    expect(toastText(en, const Phrase('peer_offline', {'name': 'PC'})), 'PC isn’t connected right now');
  });

  test('认不出的键返回空，界面改用后台附带的原文', () {
    expect(line(en, 'from_the_future'), isNull);
    expect(toastText(en, const Phrase('from_the_future')), isNull);
  });

  test('后台会发的键都认得', () {
    // 直接从后台的源码里找：后台加了新的键而这里没跟上时，这条测试会失败。
    final source = File('../crates/ferry-engine/src/resident/runtime.rs');
    final keys = {
      for (final m in RegExp(r'(?:Phrase::new|base)\("(\w+)"\)').allMatches(source.readAsStringSync())) m.group(1)!,
    };
    expect(keys, hasLength(greaterThan(40)));
    for (final key in keys) {
      final known = line(en, key) != null || toastText(en, Phrase(key)) != null;
      expect(known, isTrue, reason: '界面不认得后台的 $key');
    }
  });
}
