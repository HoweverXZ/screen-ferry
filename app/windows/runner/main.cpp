// windows.h 要在 dwmapi.h 之前。
#include <windows.h>

#include <dwmapi.h>
#include <flutter/dart_project.h>
#include <flutter/flutter_view_controller.h>
#include <flutter_windows.h>

#include <algorithm>

#include "flutter_window.h"
#include "resource.h"
#include "utils.h"

namespace {

// 占位窗口：Flutter 引擎启动、画出第一帧要一两秒，这段时间先在界面将要出现的位置
// 显示一块同样底色、中间是应用图标的窗口，点了托盘马上就有反应。第一帧画好后关掉它。
constexpr wchar_t kSplashClass[] = L"SCREENFERRY_SPLASH";
// 无论如何，过了这么久就关掉占位窗口。
constexpr UINT kSplashTimeoutMs = 15000;
// 占位窗口中间的图标边长（逻辑像素）。
constexpr int kSplashIconSide = 72;

#ifndef DWMWA_WINDOW_CORNER_PREFERENCE
#define DWMWA_WINDOW_CORNER_PREFERENCE 33
#endif
constexpr int kCornerRound = 2;

HWND g_splash = nullptr;

// 系统是不是深色模式，与界面第一帧“跟随系统”时的选择一致。
bool PrefersDark() {
  DWORD light = 1;
  DWORD size = sizeof(light);
  const LSTATUS result = ::RegGetValueW(
      HKEY_CURRENT_USER,
      L"Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
      L"AppsUseLightTheme", RRF_RT_REG_DWORD, nullptr, &light, &size);
  return result == ERROR_SUCCESS && light == 0;
}

LRESULT CALLBACK SplashProc(HWND hwnd, UINT message, WPARAM wparam,
                            LPARAM lparam) {
  switch (message) {
    case WM_ERASEBKGND:
      return 1;
    case WM_PAINT: {
      PAINTSTRUCT paint;
      HDC dc = ::BeginPaint(hwnd, &paint);
      RECT client;
      ::GetClientRect(hwnd, &client);
      // 与界面的底色相同（app/lib/theme/tokens.dart 里的 bg）。
      HBRUSH brush = ::CreateSolidBrush(PrefersDark() ? RGB(0x14, 0x13, 0x16)
                                                      : RGB(0xEF, 0xED, 0xF0));
      ::FillRect(dc, &client, brush);
      ::DeleteObject(brush);
      const int side = ::MulDiv(kSplashIconSide, static_cast<int>(::GetDpiForWindow(hwnd)), 96);
      HICON icon = static_cast<HICON>(
          ::LoadImageW(::GetModuleHandleW(nullptr),
                       MAKEINTRESOURCEW(IDI_APP_ICON), IMAGE_ICON, side, side,
                       LR_DEFAULTCOLOR));
      if (icon != nullptr) {
        ::DrawIconEx(dc, (client.right - side) / 2, (client.bottom - side) / 2,
                     icon, side, side, 0, nullptr, DI_NORMAL);
        ::DestroyIcon(icon);
      }
      ::EndPaint(hwnd, &paint);
      return 0;
    }
    case WM_TIMER:
      ::DestroyWindow(hwnd);
      return 0;
    case WM_NCDESTROY:
      if (g_splash == hwnd) {
        g_splash = nullptr;
      }
      break;
  }
  return ::DefWindowProcW(hwnd, message, wparam, lparam);
}

// 在主窗口将要出现的位置（物理像素的外框）显示占位窗口。主窗口没有标题栏，
// 左、右、下各有一圈看不见的边框（拖动改大小用），占位窗口要往里缩掉这一圈。
void ShowSplash(RECT outer, UINT dpi) {
  WNDCLASSW window_class = {};
  window_class.lpfnWndProc = SplashProc;
  window_class.hInstance = ::GetModuleHandleW(nullptr);
  window_class.hCursor = ::LoadCursorW(nullptr, IDC_APPSTARTING);
  window_class.lpszClassName = kSplashClass;
  ::RegisterClassW(&window_class);

  const int frame = ::GetSystemMetricsForDpi(SM_CXFRAME, dpi) +
                    ::GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi);
  outer.left += frame;
  outer.right -= frame;
  outer.bottom -= frame;
  // 不在任务栏上占位：主窗口出来时任务栏按钮才出现，不会一闪一换。
  g_splash = ::CreateWindowExW(
      WS_EX_TOOLWINDOW, kSplashClass, L"ScreenFerry", WS_POPUP, outer.left,
      outer.top, outer.right - outer.left, outer.bottom - outer.top, nullptr,
      nullptr, ::GetModuleHandleW(nullptr), nullptr);
  if (g_splash == nullptr) {
    return;
  }
  const int corner = kCornerRound;
  ::DwmSetWindowAttribute(g_splash, DWMWA_WINDOW_CORNER_PREFERENCE, &corner,
                          static_cast<DWORD>(sizeof(corner)));
  ::SetTimer(g_splash, 1, kSplashTimeoutMs, nullptr);
  ::ShowWindow(g_splash, SW_SHOW);
  ::UpdateWindow(g_splash);
}

void CloseSplash() {
  if (g_splash != nullptr) {
    ::DestroyWindow(g_splash);
  }
}

}  // namespace

// 设置界面只开一个：已经有一个在运行时，把它的窗口带到最前面，返回 true，新启动的这个随即退出。
// 托盘图标、菜单和双击程序都可能再启动一次。互斥量随进程退出自动释放。
static bool ActivateRunningInstance() {
  ::CreateMutexW(nullptr, TRUE, L"Local\\ScreenFerrySettings");
  if (::GetLastError() != ERROR_ALREADY_EXISTS) {
    return false;
  }
  HWND existing = ::FindWindowW(L"FLUTTER_RUNNER_WIN32_WINDOW", L"ScreenFerry");
  if (existing != nullptr && ::IsWindowVisible(existing)) {
    ::ShowWindow(existing, ::IsIconic(existing) ? SW_RESTORE : SW_SHOW);
    ::SetForegroundWindow(existing);
  } else if (HWND splash = ::FindWindowW(kSplashClass, nullptr)) {
    // 前一个还在启动，主窗口还没画好：只把占位窗口带到前面，不提前露出空白的主窗口。
    ::SetForegroundWindow(splash);
  }
  return true;
}

int APIENTRY wWinMain(_In_ HINSTANCE instance, _In_opt_ HINSTANCE prev,
                      _In_ wchar_t *command_line, _In_ int show_command) {
  if (ActivateRunningInstance()) {
    return EXIT_SUCCESS;
  }

  // Attach to console when present (e.g., 'flutter run') or create a
  // new console when running with a debugger.
  if (!::AttachConsole(ATTACH_PARENT_PROCESS) && ::IsDebuggerPresent()) {
    CreateAndAttachConsole();
  }

  // Initialize COM, so that it is available for use in the library and/or
  // plugins.
  ::CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);

  flutter::DartProject project(L"data");

  std::vector<std::string> command_line_arguments =
      GetCommandLineArguments();

  project.set_dart_entrypoint_arguments(std::move(command_line_arguments));

  // 与 macOS 相同的默认大小，放在主屏幕工作区的中间。
  const int width = 1160;
  const int height = 780;
  RECT work = {};
  ::SystemParametersInfo(SPI_GETWORKAREA, 0, &work, 0);
  const double scale = ::GetDpiForSystem() / 96.0;
  const int work_width = static_cast<int>((work.right - work.left) / scale);
  const int work_height = static_cast<int>((work.bottom - work.top) / scale);
  Win32Window::Point origin(
      static_cast<unsigned int>(
          (std::max)(0, static_cast<int>(work.left / scale) +
                            (work_width - width) / 2)),
      static_cast<unsigned int>(
          (std::max)(0, static_cast<int>(work.top / scale) +
                            (work_height - height) / 2)));
  Win32Window::Size size(width, height);

  // 与 Win32Window::Create 相同的换算，得到主窗口的外框。
  const POINT target = {static_cast<LONG>(origin.x),
                        static_cast<LONG>(origin.y)};
  const UINT dpi = FlutterDesktopGetDpiForMonitor(
      ::MonitorFromPoint(target, MONITOR_DEFAULTTONEAREST));
  const double monitor_scale = dpi / 96.0;
  const int left = static_cast<int>(origin.x * monitor_scale);
  const int top = static_cast<int>(origin.y * monitor_scale);
  ShowSplash(RECT{left, top, left + static_cast<int>(width * monitor_scale),
                  top + static_cast<int>(height * monitor_scale)},
             dpi);

  FlutterWindow window(project);
  // 第一帧画好、主窗口出来之后再关掉占位窗口，中间不留空档。
  window.SetFirstFrameCallback(CloseSplash);
  if (!window.Create(L"ScreenFerry", origin, size)) {
    return EXIT_FAILURE;
  }
  window.SetQuitOnClose(true);

  ::MSG msg;
  while (::GetMessage(&msg, nullptr, 0, 0)) {
    ::TranslateMessage(&msg);
    ::DispatchMessage(&msg);
  }

  ::CoUninitialize();
  return EXIT_SUCCESS;
}
