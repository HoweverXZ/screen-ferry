// 按地址连接、邀请没成时给用户看的话：标题说发生了什么，说明告诉用户可以怎么办。

import '../ipc/messages.dart';
import '../l10n/l10n.dart';

/// 一次没成的按地址连接或邀请。
({String title, String body}) dialFailure(
  AppLocalizations l,
  DialError error, {
  required String address,
  required bool invite,
}) => switch (error) {
  DialError.badAddress => (title: l.dialBadAddressTitle(address), body: l.dialBadAddressBody),
  DialError.refused when invite => (title: l.dialNotWaitingInviteTitle(address), body: l.dialNotWaitingInviteBody),
  DialError.refused => (title: l.dialNotWaitingTitle(address), body: l.dialNotWaitingBody),
  DialError.unreachable => (title: l.dialUnreachableTitle(address), body: l.dialUnreachableBody),
  DialError.notScreenFerry => (title: l.dialNotScreenFerryTitle(address), body: l.dialNotScreenFerryBody),
  DialError.incompatible => (title: l.dialIncompatibleTitle, body: l.dialIncompatibleBody),
  DialError.busy => (title: l.dialBusyTitle(address), body: l.dialBusyBody),
  DialError.wrongRole when invite => (title: l.dialWrongRoleInviteTitle, body: l.dialWrongRoleInviteBody),
  DialError.wrongRole => (title: l.dialWrongRoleTitle, body: l.dialWrongRoleBody),
};
