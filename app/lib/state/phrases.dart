// 后台发来的活动记录和提示带着一句结构化的话（Phrase：键加参数），这里按当前语言说出来（ADR 0013）。
// 认不出的键返回 null，界面改用后台附带的中文原文：后台比界面新时也不会显示空白。

import '../l10n/l10n.dart';
import 'app_state.dart';

/// 活动记录的标题和说明。[mac]：本机是不是 Mac，快捷键按它的叫法写。
(String, String)? logLine(AppLocalizations l, Phrase p, {required bool mac}) {
  final name = p.arg('name');
  return switch (p.key) {
    'started' => (l.phStarted, p.arg('role') == 'client' ? l.phStartedClient(name) : l.phStartedServer(name)),
    'cannot_share' => (l.phCannotShare, _problem(l, p)),
    'listening' => (
      l.phListening,
      p.flag('announced') ? l.phListeningAnnounced(p.arg('port')) : l.phListeningQuiet(p.arg('port')),
    ),
    'connected' => (l.phConnected(name), 'TLS 1.3 · ${p.arg('ip')}'),
    'disconnected' => (l.phDisconnected(name), p.args['reason'] ?? l.phPeerClosed),
    'peer_opened_settings' => (l.phPeerOpenedSettings(name), l.phShowingItsSettings),
    'cursor_to_peer' => (l.phCursorToPeer(name), l.phAcrossEdge(p.arg('side'))),
    'cursor_back' => (
      l.phCursorBack,
      p.flag('hotkey') ? l.phShortcut('Ctrl + ${mac ? 'Option' : 'Alt'} + Esc') : l.phFromPeerEdge,
    ),
    'cursor_here' => (l.phCursorHere, l.phFrom(name)),
    'cursor_returned' => (l.phCursorReturned(name), ''),
    'clipboard_synced' => (p.flag('to_peer') ? l.phClipToPeer(name) : l.phClipHere, _clip(l, p)),
    'clipboard_skipped' => (
      p.flag('to_peer') ? l.phClipTooBigToPeer(p.arg('format'), name) : l.phClipTooBigHere(p.arg('format'), name),
      l.phClipLimit(sizeText(p.number('bytes')), sizeText(p.number('limit'))),
    ),
    'paired' => (l.phPaired(name), l.devicesFingerprintShort(p.arg('short'))),
    'pairing_rejected' => (p.flag('by_peer') ? l.phRejectedByPeer(name) : l.phRejected(name), ''),
    'pairing_request' => (l.phPairingRequest(name), l.phPairingCode(p.arg('code'))),
    'transfer_sending' => (l.phSending(name, _what(l, p)), _files(l, p)),
    'transfer_receiving' => (l.phReceiving(name, _what(l, p)), _files(l, p)),
    'transfer_sent' => (l.phSent(_what(l, p)), l.phSentTo(name, sizeText(p.number('bytes')))),
    'transfer_received' => (l.phReceived(_what(l, p)), p.args['folder'] == null ? '' : l.phSavedIn(p.arg('folder'))),
    'transfer_interrupted' => (l.phInterrupted(_what(l, p)), l.phResumesLater),
    'transfer_cancelled' => (l.phCancelled(_what(l, p)), p.flag('by_peer') ? l.phCancelledByPeer : l.phCancelledHere),
    'transfer_failed' => (l.phFailed(_what(l, p)), p.arg('reason')),
    'sharing_resumed' => (l.phSharingResumed, l.phSharingResumedHint),
    'sharing_paused' => (l.phSharingPaused, l.phSharingPausedHint),
    'layout_arranged' => (l.phArranged, l.phLayoutSaved),
    'layout_moved' => (
      l.phMoved(name.isEmpty ? l.phOtherDevices : name, l.layoutWhere(p.arg('side'))),
      l.phLayoutSaved,
    ),
    'role_changed' =>
      p.arg('role') == 'client' ? (l.phNowClient, l.phNowClientHint) : (l.phNowServer, l.phNowServerHint),
    'removed' => (l.phRemoved(name), l.phPairAgain),
    _ => null,
  };
}

/// 提示的文字。
String? toastText(AppLocalizations l, Phrase p) {
  final name = p.arg('name');
  final message = p.arg('message');
  return switch (p.key) {
    'peer_settings_opened' => l.toastPeerSettingsOpened(name),
    'peer_settings_unsupported' => l.toastPeerSettingsUnsupported(name),
    'paired' => l.phPaired(name),
    'diagnostics_missing' => l.toastDiagnosticsMissing,
    'not_connected' => l.toastNotConnectedPeer,
    'files_disabled' => l.toastFilesDisabled(name),
    'sending' => l.toastSending(name),
    'error' => message,
    'sharing_resumed' => l.toastSharingResumed,
    'sharing_paused' => l.toastSharingPaused,
    'autostart_failed' => l.toastAutostartFailed(message),
    'save_failed' => l.toastSaveFailed(message),
    'peer_offline' => l.toastPeerOffline(name),
    'peers_read_failed' => l.toastPeersReadFailed(message),
    'remove_failed' => l.toastRemoveFailed(message),
    'removed' => l.phRemoved(name),
    'server_gone' => l.toastServerGone,
    'paused_connect_later' => l.toastPausedConnectLater,
    'not_sharing_invite_later' => l.toastNotSharingInviteLater,
    'diagnostics_failed' => l.toastDiagnosticsFailed(message),
    _ => null,
  };
}

String _problem(AppLocalizations l, Phrase p) => switch (p.arg('problem')) {
  'permission' => l.problemPermission,
  'port_in_use' when p.args['port'] != null => l.problemPortInUse(p.number('port')),
  _ => p.arg('message'),
};

String _clip(AppLocalizations l, Phrase p) => switch (p.arg('content')) {
  'image' => l.overviewClipImage(p.number('width'), p.number('height'), sizeText(p.number('bytes'))),
  'rich' => l.overviewClipRich(p.number('chars')),
  'text' => l.overviewClipText(p.number('chars')),
  _ => sizeText(p.number('bytes')),
};

/// 传了什么：一项时是它的名字，多项时是“第一项等 N 项”。
String _what(AppLocalizations l, Phrase p) {
  final items = p.number('items');
  final file = p.arg('file');
  return items > 1 ? l.phItems(file, items, items - 1) : file;
}

String _files(AppLocalizations l, Phrase p) => l.phFiles(p.number('files'), sizeText(p.number('bytes')));
