//! 应用主流程：按平台选择实现，启动引擎。

use std::convert::Infallible;
use std::io::IsTerminal;
use std::net::SocketAddr;
use std::process::ExitCode;

use ferry_core::{KeyRemap, Side};
use tracing::{error, info};

use crate::cli::{Mode, Options};

type AppResult = Result<Infallible, Box<dyn std::error::Error>>;

pub(crate) fn init_logging(verbose: bool) {
    let level = if verbose {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };
    tracing_subscriber::fmt()
        .with_max_level(level)
        .with_target(false)
        .with_timer(tracing_subscriber::fmt::time::uptime())
        .with_ansi(std::io::stderr().is_terminal())
        .with_writer(std::io::stderr)
        .init();
}

pub(crate) fn run(options: Options) -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => {
            error!("无法启动运行时：{e}");
            return ExitCode::FAILURE;
        }
    };
    let result = runtime.block_on(async move {
        let work = async {
            match options.mode {
                Mode::Serve {
                    listen,
                    side,
                    swap_cmd_ctrl,
                } => {
                    let remap = if swap_cmd_ctrl {
                        KeyRemap::swap_command_control()
                    } else {
                        KeyRemap::none()
                    };
                    serve(listen, side, remap, options.name).await
                }
                Mode::Connect { server } => connect(server, options.name).await,
            }
        };
        // 收到 Ctrl+C 时让引擎正常析构：归还鼠标控制、松开按键，而不是被系统直接杀掉。
        tokio::select! {
            result = work => result.map(|never| match never {}),
            _ = tokio::signal::ctrl_c() => {
                info!("收到 Ctrl+C，正在退出");
                Ok(())
            }
        }
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            error!("{e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(target_os = "macos")]
async fn serve(listen: SocketAddr, side: Side, remap: KeyRemap, name: String) -> AppResult {
    use ferry_engine::ServerConfig;
    use ferry_platform::Permissions;
    use ferry_platform_macos::{MacCapture, MacDisplays, MacPermissions};

    MacPermissions.check_input()?;
    let listener = tokio::net::TcpListener::bind(listen).await?;
    let config = ServerConfig { side, name, remap };
    Ok(ferry_engine::serve(listener, config, MacCapture::new(), MacDisplays).await?)
}

#[cfg(not(target_os = "macos"))]
async fn serve(_listen: SocketAddr, _side: Side, _remap: KeyRemap, _name: String) -> AppResult {
    Err("原型阶段只能在 macOS 上运行 serve".into())
}

#[cfg(target_os = "windows")]
async fn connect(server: SocketAddr, name: String) -> AppResult {
    use ferry_engine::ClientConfig;
    use ferry_platform::Permissions;
    use ferry_platform_windows::{WinDisplays, WinInject, WinPermissions};

    WinPermissions.check_input()?;
    let config = ClientConfig { server, name };
    Ok(ferry_engine::connect(config, WinInject::new(), WinDisplays::new()).await?)
}

#[cfg(not(target_os = "windows"))]
async fn connect(_server: SocketAddr, _name: String) -> AppResult {
    Err("原型阶段只能在 Windows 上运行 connect".into())
}
