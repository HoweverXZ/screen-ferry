import 'package:flutter/widgets.dart';

import 'app_state.dart';

/// 向下提供 [AppState]；读取它的组件在状态变化时自动重建。
class AppScope extends InheritedNotifier<AppState> {
  const AppScope({super.key, required AppState state, required super.child}) : super(notifier: state);

  static AppState of(BuildContext context) => context.dependOnInheritedWidgetOfExactType<AppScope>()!.notifier!;

  /// 只取状态、不订阅变化，用在事件回调里。
  static AppState read(BuildContext context) => context.getInheritedWidgetOfExactType<AppScope>()!.notifier!;
}
