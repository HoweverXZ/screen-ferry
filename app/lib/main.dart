import 'package:flutter/material.dart';

import 'app.dart';
import 'state/app_state.dart';

/// `--dart-define=SF_DEMO=true` 时用演示数据，不连后台，方便单独看界面。
const _demo = bool.fromEnvironment('SF_DEMO');

void main() {
  WidgetsFlutterBinding.ensureInitialized();
  final state = AppState(backend: _demo ? DemoBackend() : IpcBackend())..start();
  runApp(ScreenFerryApp(state: state));
}
