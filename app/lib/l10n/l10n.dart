// 多语言（ADR 0013）：语言文件是同目录下的 app_zh.arb（源语言）和 app_en.arb，
// flutter pub get 按 l10n.yaml 生成 app_localizations*.dart。

import 'package:flutter/widgets.dart';

import '../ipc/messages.dart';
import 'app_localizations.dart';

export 'app_localizations.dart';

extension L10nContext on BuildContext {
  /// 当前语言的文字。
  AppLocalizations get l10n => AppLocalizations.of(this);
}

/// 设置里选的语言换成实际用的：跟随系统时看系统的首选语言，中文都用简体中文，其他用英文。
Locale resolveLocale(UiLanguage language, Locale system) => switch (language) {
  UiLanguage.zh => const Locale('zh'),
  UiLanguage.en => const Locale('en'),
  UiLanguage.system => system.languageCode == 'zh' ? const Locale('zh') : const Locale('en'),
};
