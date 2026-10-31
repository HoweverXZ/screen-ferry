# 0003. 设置界面改用 Flutter

- 状态：已采纳（2026-11-01，维护者审阅 HTML 设计稿与 Flutter 原型后确认）；通信方式部分被 ADR 0004 取代
- 日期：2026-11-01

## 背景

AGENTS.md 第 2 节原定设置界面用 egui。评估过程中维护者明确希望界面达到 FlClash 那种 Material You 风格与动效。候选方案：

| 方案 | 优点 | 缺点 |
|---|---|---|
| egui | 纯 Rust，体积小 | Material 风格与动效要逐帧手写；破坏性升级频繁 |
| Slint | 纯 Rust，内存小，声明式动画，1.x 内不破坏 | Windows 10 上切换到微软拼音时界面冻结的问题至今未修（slint#5206）；许可证须注明使用 Slint |
| 两端原生（SwiftUI + WinUI 3） | 真正原生 | 两套代码、两种语言；Windows 版无法在 Mac 上构建 |
| Flutter | Material 3 原生支持，动效最好；中文输入法成熟 | 多一套 Dart 工具链；Windows 版须在 Windows 上构建 |

## 决策

设置界面用 Flutter，其余部分不变：

- 后台 `screenferry`（Rust）照旧常驻，负责键鼠、剪贴板、网络、托盘与配对确认，不依赖 Flutter。
- Flutter 程序只在打开设置时启动，关闭即退出。
- 界面与后台的通信经由 `ferry-settings`：一个只依赖 `ferry-ipc` 的 Rust 库，通过 flutter_rust_bridge 编进 Flutter 程序。通信协议只在 Rust 中定义一次。

## 对规范的修改

- 第 2 节：界面一行改为 Flutter（自定义设计，见下文“设计与实现约定”）。
- 第 3 节：顶层新增 `app/`，存放 Flutter 工程；它不是 Cargo workspace 成员。
- 第 3 节：`assets/` 是唯一存放二进制资源的地方，但平台工程要求放在固定位置的图标例外，例如 `app/macos/Runner/Assets.xcassets/AppIcon.appiconset/`。目前是 Flutter 生成的占位图；正式图标的源文件放 `assets/icons/`，导出后覆盖这些文件。
- 第 4 节：`ferry-settings` 从可执行文件改为 Flutter 调用的 Rust 库，层级仍为 L4，仍只依赖 `ferry-ipc`。
- 第 12 节：改动 `app/` 时，提交前另外运行 `dart format`、`flutter analyze`、`flutter test`（在 `app/` 中运行）。

## 设计与实现约定

- 视觉以已确认的 HTML 设计稿为准：暖灰底、近白卡片、连续曲率圆角、iOS 风格开关、滑块式分段控件；仪表盘主卡以旋转、呼吸的花朵表示“正在共享”。
- 颜色全部来自 `app/lib/theme/tokens.dart` 的设计令牌，浅色、深色各一套，组件不写死颜色。
- 动效只做开销小的变换。循环动画（花朵）按每秒 30 帧刷新、只重绘自身，并在暂停共享、窗口不在前台或系统要求减少动效时停下。
- 窗口最小 375×667（手机大小）。窄于 560 时侧栏换成底部标签栏；放不下的行尾控件换到标题下面。
- 两个平台的界面完全一致，只有两处按平台习惯：
  - 窗口按钮：macOS 用系统的红绿灯；Windows 去掉系统标题栏，界面在右上角画最小化、最大化、关闭（`app/lib/shell/window_frame.dart`）。两边顶部 32 点都留给窗口按钮与拖动，页面布局不变。
  - 字体：各用系统字体，macOS 苹方，Windows 微软雅黑；等宽字体 Menlo / Consolas。
- 平台差异尽量写在共享的 Dart 代码里（`Platform.isWindows`）。`app/macos`、`app/windows` 是 Flutter 编译所需的宿主工程，只改窗口外观、最小尺寸和权限这几处。
- 除 Flutter SDK 外暂不引入第三方 Dart 包。

## 后果

- 打开设置时多占约 150 MB 内存（实测：5K 屏、窗口 1160×780，大头是 GPU 显存），关闭即释放；常驻部分不受影响。窗口在前台时 CPU 约 7–8%，转到后台后约 1.5%。
- CI 需要 Flutter 构建任务；Windows 版必须在 Windows 上构建。
- FlClash 为 GPL-3.0，只借鉴设计思路，不复制代码。
