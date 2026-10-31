//! ferry-platform-macos 的测试入口。
//!
//! 测试按 `src/` 的模块结构在本目录下镜像组织，见 AGENTS.md 第 9 节。
//! 捕获需要“辅助功能”权限和真实的键鼠操作，不在自动测试范围内。

#![cfg(target_os = "macos")]

mod autostart;
mod display;
mod keymap;
