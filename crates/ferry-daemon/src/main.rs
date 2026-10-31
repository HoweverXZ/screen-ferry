//! screenferry：后台常驻进程，负责键鼠共享、剪贴板与文件同步、托盘。
//!
//! 层级：L4。按目标平台选择 `ferry-platform-*` 实现并注入 `ferry-engine`。

#![forbid(unsafe_code)]

mod app;
mod cli;
mod pairing;

use std::process::ExitCode;

fn main() -> ExitCode {
    let options = match cli::parse(std::env::args().skip(1)) {
        Ok(cli::Parsed::Run(options)) => options,
        Ok(cli::Parsed::Help) => {
            println!("{}", cli::USAGE);
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("{e}\n\n{}", cli::USAGE);
            return ExitCode::from(2);
        }
    };
    app::init_logging(options.verbose);
    app::run(options)
}
