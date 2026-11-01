// 按地址连接、邀请没成时给用户看的话：标题说发生了什么，说明告诉用户可以怎么办。

import '../ipc/messages.dart';

/// 一次没成的按地址连接或邀请。
({String title, String body}) dialFailure(DialError error, {required String address, required bool invite}) =>
    switch (error) {
      DialError.badAddress => (title: '地址不对：$address', body: '填 IP（例如 192.168.2.12）、IP:端口，或者电脑名。'),
      DialError.refused when invite => (
        title: '$address 没有在等待邀请',
        body: '确认那台电脑开着 ScreenFerry，并且选的是“被控制”。它的版本太旧时也不能被邀请，需要先升级。',
      ),
      DialError.refused => (title: '$address 没有在等待连接', body: '确认那台电脑开着 ScreenFerry，并且在设置里选的是“控制别的电脑”。'),
      DialError.unreachable => (
        title: '连不上 $address',
        body: '确认两台在同一个网络里、地址没有填错。Windows 第一次运行 ScreenFerry 时会询问防火墙，请允许它通过。',
      ),
      DialError.notScreenFerry => (title: '$address 上不是 ScreenFerry', body: '确认地址和端口没有填错。'),
      DialError.incompatible => (title: '版本不兼容', body: '两台电脑上的 ScreenFerry 版本相差太多，请都升级到最新版。'),
      DialError.busy => (title: '$address 正连着另一台电脑', body: '先在那台电脑上断开，再邀请。'),
      DialError.wrongRole when invite => (title: '本机现在是被控制的一方', body: '邀请是控制端用的；被控制的一方请用“按地址连接”。'),
      DialError.wrongRole => (title: '本机现在是控制端', body: '按地址连接是被控制的一方用的；控制端请用“邀请一台电脑”。'),
    };
