#include <flutter/dart_project.h>
#include <flutter/flutter_view_controller.h>
#include <windows.h>

#include <algorithm>

#include "flutter_window.h"
#include "utils.h"

int APIENTRY wWinMain(_In_ HINSTANCE instance, _In_opt_ HINSTANCE prev,
                      _In_ wchar_t *command_line, _In_ int show_command) {
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

  FlutterWindow window(project);
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
