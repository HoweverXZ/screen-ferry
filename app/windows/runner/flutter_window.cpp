#include "flutter_window.h"

#include <dwmapi.h>
#include <flutter/standard_method_codec.h>

#include <optional>

#include "flutter/generated_plugin_registrant.h"

namespace {

// 窗口的最小尺寸（逻辑像素），与 macOS 一致：手机大小。
constexpr int kMinWidth = 375;
constexpr int kMinHeight = 667;

// Windows 11 的圆角窗口。旧版 SDK 里没有这些定义。
#ifndef DWMWA_WINDOW_CORNER_PREFERENCE
#define DWMWA_WINDOW_CORNER_PREFERENCE 33
#endif
constexpr int kCornerRound = 2;

}  // namespace

FlutterWindow::FlutterWindow(const flutter::DartProject& project)
    : project_(project) {}

FlutterWindow::~FlutterWindow() {}

bool FlutterWindow::OnCreate() {
  if (!Win32Window::OnCreate()) {
    return false;
  }

  RECT frame = GetClientArea();

  // The size here must match the window dimensions to avoid unnecessary surface
  // creation / destruction in the startup path.
  flutter_controller_ = std::make_unique<flutter::FlutterViewController>(
      frame.right - frame.left, frame.bottom - frame.top, project_);
  // Ensure that basic setup of the controller was successful.
  if (!flutter_controller_->engine() || !flutter_controller_->view()) {
    return false;
  }
  RegisterPlugins(flutter_controller_->engine());
  SetChildContent(flutter_controller_->view()->GetNativeWindow());

  HWND hwnd = GetHandle();
  // 顶部向客户区延伸 1 像素，去掉标题栏后仍保留系统阴影；Windows 11 上用圆角。
  const MARGINS margins = {0, 0, 1, 0};
  DwmExtendFrameIntoClientArea(hwnd, &margins);
  const int corner = kCornerRound;
  DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &corner,
                        static_cast<DWORD>(sizeof(corner)));
  // 让 WM_NCCALCSIZE 立即生效，去掉标题栏。
  SetWindowPos(hwnd, nullptr, 0, 0, 0, 0,
               SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER |
                   SWP_NOACTIVATE);

  window_channel_ =
      std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
          flutter_controller_->engine()->messenger(), "screenferry/window",
          &flutter::StandardMethodCodec::GetInstance());
  window_channel_->SetMethodCallHandler(
      [this](const flutter::MethodCall<flutter::EncodableValue>& call,
             std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>>
                 result) {
        HWND window = GetHandle();
        const std::string& method = call.method_name();
        if (method == "minimize") {
          ShowWindow(window, SW_MINIMIZE);
        } else if (method == "toggleMaximize") {
          ShowWindow(window, IsZoomed(window) ? SW_RESTORE : SW_MAXIMIZE);
        } else if (method == "close") {
          PostMessage(window, WM_CLOSE, 0, 0);
        } else if (method == "startDrag") {
          // 交给系统按“拖标题栏”处理：移动、贴边分屏都和普通窗口一样。
          ReleaseCapture();
          SendMessage(window, WM_NCLBUTTONDOWN, HTCAPTION, 0);
        } else if (method == "isMaximized") {
          result->Success(flutter::EncodableValue(IsZoomed(window) != 0));
          return;
        } else {
          result->NotImplemented();
          return;
        }
        result->Success();
      });

  flutter_controller_->engine()->SetNextFrameCallback([&]() {
    this->Show();
  });

  // Flutter can complete the first frame before the "show window" callback is
  // registered. The following call ensures a frame is pending to ensure the
  // window is shown. It is a no-op if the first frame hasn't completed yet.
  flutter_controller_->ForceRedraw();

  return true;
}

void FlutterWindow::OnDestroy() {
  window_channel_ = nullptr;
  if (flutter_controller_) {
    flutter_controller_ = nullptr;
  }

  Win32Window::OnDestroy();
}

LRESULT
FlutterWindow::MessageHandler(HWND hwnd, UINT const message,
                              WPARAM const wparam,
                              LPARAM const lparam) noexcept {
  switch (message) {
    case WM_NCCALCSIZE:
      if (wparam != 0) {
        // 去掉标题栏：先让系统算出左、右、下的边框（保留拖动改大小），再把顶部还原到窗口顶端。
        auto params = reinterpret_cast<NCCALCSIZE_PARAMS*>(lparam);
        const LONG top = params->rgrc[0].top;
        DefWindowProc(hwnd, message, wparam, lparam);
        params->rgrc[0].top = top;
        if (IsZoomed(hwnd)) {
          // 最大化时窗口比屏幕大出一圈边框，顶部要让出来，内容才不会被裁掉。
          const UINT dpi = GetDpiForWindow(hwnd);
          params->rgrc[0].top += GetSystemMetricsForDpi(SM_CYFRAME, dpi) +
                                 GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi);
        }
        return 0;
      }
      break;
    case WM_GETMINMAXINFO: {
      // 最小尺寸按当前 DPI 换算，再加上左右下的边框。
      const UINT dpi = GetDpiForWindow(hwnd);
      const double scale = dpi / 96.0;
      const int frame = GetSystemMetricsForDpi(SM_CXFRAME, dpi) +
                        GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi);
      auto info = reinterpret_cast<MINMAXINFO*>(lparam);
      info->ptMinTrackSize.x = static_cast<LONG>(kMinWidth * scale) + frame * 2;
      info->ptMinTrackSize.y = static_cast<LONG>(kMinHeight * scale) + frame;
      return 0;
    }
    case WM_SIZE: {
      const bool maximized = wparam == static_cast<WPARAM>(SIZE_MAXIMIZED);
      if (window_channel_ && maximized != maximized_ &&
          wparam != static_cast<WPARAM>(SIZE_MINIMIZED)) {
        maximized_ = maximized;
        window_channel_->InvokeMethod(
            "maximized", std::make_unique<flutter::EncodableValue>(maximized));
      }
      break;
    }
  }

  // Give Flutter, including plugins, an opportunity to handle window messages.
  if (flutter_controller_) {
    std::optional<LRESULT> result =
        flutter_controller_->HandleTopLevelWindowProc(hwnd, message, wparam,
                                                      lparam);
    if (result) {
      return *result;
    }
  }

  switch (message) {
    case WM_FONTCHANGE:
      flutter_controller_->engine()->ReloadSystemFonts();
      break;
  }

  return Win32Window::MessageHandler(hwnd, message, wparam, lparam);
}
