// 各测试共用的辅助代码。

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';

/// 把系统语言固定为 [locale]。测试默认按中文的界面写，英文的界面另有测试。
void useLocale(WidgetTester tester, [Locale locale = const Locale('zh')]) {
  tester.platformDispatcher.localeTestValue = locale;
  tester.platformDispatcher.localesTestValue = [locale];
  addTearDown(tester.platformDispatcher.clearLocaleTestValue);
  addTearDown(tester.platformDispatcher.clearLocalesTestValue);
}
