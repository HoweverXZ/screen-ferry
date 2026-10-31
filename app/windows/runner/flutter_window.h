#ifndef RUNNER_FLUTTER_WINDOW_H_
#define RUNNER_FLUTTER_WINDOW_H_

#include <flutter/dart_project.h>
#include <flutter/encodable_value.h>
#include <flutter/flutter_view_controller.h>
#include <flutter/method_channel.h>

#include <memory>

#include "win32_window.h"

// 承载 Flutter 界面的窗口。没有系统标题栏：窗口按钮和拖动区域由 Flutter 画，
// 经 "screenferry/window" 通道让这里执行最小化、最大化、关闭、拖动。
class FlutterWindow : public Win32Window {
 public:
  // Creates a new FlutterWindow hosting a Flutter view running |project|.
  explicit FlutterWindow(const flutter::DartProject& project);
  virtual ~FlutterWindow();

 protected:
  // Win32Window:
  bool OnCreate() override;
  void OnDestroy() override;
  LRESULT MessageHandler(HWND window, UINT const message, WPARAM const wparam,
                         LPARAM const lparam) noexcept override;

 private:
  // The project to run.
  flutter::DartProject project_;

  // The Flutter instance hosted by this window.
  std::unique_ptr<flutter::FlutterViewController> flutter_controller_;

  // 与 Flutter 之间的窗口控制通道。
  std::unique_ptr<flutter::MethodChannel<flutter::EncodableValue>> window_channel_;

  // 上次告诉 Flutter 的最大化状态。
  bool maximized_ = false;
};

#endif  // RUNNER_FLUTTER_WINDOW_H_
