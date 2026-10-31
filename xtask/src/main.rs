//! 仓库自动化任务。用法：`cargo xtask <命令>`。

#![forbid(unsafe_code)]

mod deps;
mod layout;
mod workspace;

use std::process::ExitCode;

const USAGE: &str = "\
用法：cargo xtask <命令>

命令：
  lint    检查仓库是否符合 AGENTS.md 中的结构规则与依赖规则";

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("lint") => lint(),
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
