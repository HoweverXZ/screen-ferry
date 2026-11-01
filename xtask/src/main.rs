//! 仓库自动化任务。用法：`cargo xtask <命令>`。

#![forbid(unsafe_code)]

mod bundle;
mod deps;
mod icons;
mod layout;
mod workspace;

use std::process::ExitCode;

const USAGE: &str = "\
用法：cargo xtask <命令>

命令：
  lint    检查仓库是否符合 AGENTS.md 中的结构规则与依赖规则
  bundle  打包：macOS 上生成 target/bundle/macos/ScreenFerry.app（后台 + 设置界面）
  icons   从 assets/icons/ 的 SVG 导出托盘图标和两个系统的应用图标（需要 rsvg-convert）";

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("lint") => lint(),
        Some("bundle") => match bundle::run() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("打包失败：{e}");
                ExitCode::FAILURE
            }
        },
        Some("icons") => match icons::run() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("导出图标失败：{e}");
                ExitCode::FAILURE
            }
        },
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn lint() -> ExitCode {
    let ws = match workspace::load() {
        Ok(ws) => ws,
        Err(e) => {
            eprintln!("读取 workspace 失败：{e}");
            return ExitCode::FAILURE;
        }
    };

    let mut violations = Vec::new();
    layout::check(&ws, &mut violations);
    deps::check(&ws, &mut violations);

    if violations.is_empty() {
        println!("lint 通过：{} 个 crate 均符合 AGENTS.md", ws.crates.len());
        return ExitCode::SUCCESS;
    }
    for v in &violations {
        eprintln!("✗ {v}");
    }
    eprintln!("\n共 {} 处违反 AGENTS.md", violations.len());
    ExitCode::FAILURE
}
