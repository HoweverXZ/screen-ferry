# Changelog

格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [Unreleased]

### 新增

- workspace 骨架：14 个 crate、`cargo xtask lint` 规范检查、CI。
- `ferry-types`：HID 键码、按键动作、锁定键、鼠标按键、几何、显示器、输入事件、剪贴板格式。
- `ferry-proto`：线协议版本 1。帧编解码、握手与能力协商、心跳、屏幕切换、输入事件、剪贴板通告与拉取、分块传输。
- 协议解码器的 fuzz 目标。
