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
