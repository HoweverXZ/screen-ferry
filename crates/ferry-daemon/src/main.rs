//! screenferry：后台常驻进程，负责键鼠共享、剪贴板与文件同步、托盘。
//!
//! 层级：L4。按目标平台选择 `ferry-platform-*` 实现并注入 `ferry-engine`。

#![forbid(unsafe_code)]
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod app;

fn main() -> std::process::ExitCode {
    app::run()
}
