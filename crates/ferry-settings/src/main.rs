//! screenferry-settings：设置界面，按需启动，关闭即退出。
//!
//! 层级：L4。只通过 `ferry-ipc` 与后台通信，不直接读写配置。

#![forbid(unsafe_code)]
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod app;

fn main() -> std::process::ExitCode {
    app::run()
}
