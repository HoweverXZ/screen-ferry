import 'package:flutter/material.dart';

import 'app.dart';
import 'state/app_state.dart';

void main() {
  final state = AppState()..start();
  runApp(ScreenFerryApp(state: state));
}
