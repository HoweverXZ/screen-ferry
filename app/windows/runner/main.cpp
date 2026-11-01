#include <flutter/dart_project.h>
#include <flutter/flutter_view_controller.h>
#include <windows.h>

#include <algorithm>

#include "flutter_window.h"
#include "utils.h"

// 设置界面只开一个：已经有一个在运行时，把它的窗口带到最前面，返回 true，新启动的这个随即退出。
// 托盘图标、菜单和双击程序都可能再启动一次。互斥量随进程退出自动释放。
static bool ActivateRunningInstance() {
  ::CreateMutexW(nullptr, TRUE, L"Local\\ScreenFerrySettings");
  if (::GetLastError() != ERROR_ALREADY_EXISTS) {
    return false;
  }
  // 主窗口创建后立即显示（还在启动时显示底色和图标），找到就带到前面。
  HWND existing = ::FindWindowW(L"FLUTTER_RUNNER_WIN32_WINDOW", L"ScreenFerry");
  if (existing != nullptr) {
    ::ShowWindow(existing, ::IsIconic(existing) ? SW_RESTORE : SW_SHOW);
    ::SetForegroundWindow(existing);
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

  FlutterWindow window(project);
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
