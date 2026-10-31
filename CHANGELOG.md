# Changelog

格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [Unreleased]

### 新增

- workspace 骨架：14 个 crate、`cargo xtask lint` 规范检查、CI。
- `ferry-types`：HID 键码、按键动作、锁定键、鼠标按键、几何、显示器、输入事件、剪贴板格式。
- `ferry-proto`：线协议版本 1。帧编解码、握手与能力协商、心跳、屏幕切换、输入事件、剪贴板通告与拉取、分块传输。
- 协议解码器的 fuzz 目标。
- 端到端验证原型：Mac 捕获鼠标，经 TCP 发给 Windows 注入。
  - `ferry-platform`：捕获、注入、显示器、权限四个 trait；`ferry-platform-mock` 提供测试实现。
  - `ferry-core`：边缘切换（四个方向、按比例换算、缩放余数累积、拖拽中不切换）与按键状态跟踪。
  - `ferry-net`：在任意字节流上收发协议帧，握手流程，TCP 连接。
  - `ferry-platform-macos`：CGEventTap 捕获鼠标、光标锁定、显示器、辅助功能权限检查。
  - `ferry-platform-windows`：`SendInput` 注入鼠标、显示器枚举、每显示器 DPI 感知。
  - `ferry-engine`：服务端与客户端会话、心跳与延迟统计、断线时归还控制并松开按键。
  - `screenferry serve` / `screenferry connect` 命令行。
- 键盘共享：
  - macOS 键码 → HID、HID → Windows 扫描码两张对照表；Windows 以扫描码注入，不受布局和输入法影响。
  - macOS 捕获键盘，修饰键按左右分别识别；抓取前按下的键，松开仍交给本机。
  - 默认互换 Command 与 Ctrl（`--no-cmd-ctrl-swap` 关闭）。
  - 进入对方屏幕时同步 Caps Lock。
  - Ctrl + Option + Esc 强制回到本机。
- 剪贴板同步（纯文本，双向）：
  - 每对设备之间增加一条数据连接，带着输入连接的会话号建立；它失败不影响键鼠。
  - 光标所在的一方通告剪贴板变化，光标过去时对方才拉取；复制多次只传最后一次。
  - 自己写入的内容不会被再次通告，连接时也不会用对方的旧内容覆盖本机。
  - macOS 基于 NSPasteboard，Windows 基于 Win32 剪贴板（`\r\n` 与 `\n` 互转）。
  - `--no-clipboard` 关闭。
