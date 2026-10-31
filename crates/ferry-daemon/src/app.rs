//! 应用主流程：按平台选择实现，启动引擎。

use std::convert::Infallible;
use std::io::IsTerminal;
use std::net::SocketAddr;
use std::process::ExitCode;

use ferry_core::KeyRemap;
use ferry_engine::{ClientConfig, Events, Live, ServerAddr, ServerConfig};
use ferry_store::Store;
use ferry_trust::Fingerprint;
use tracing::{error, info};

use crate::cli::{Mode, Options};
#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::pairing::TerminalConfirm;

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
    let store = match &options.data_dir {
        Some(dir) => Store::open(dir),
        None => Store::open_default(),
    };
    let store = match store {
        Ok(store) => store,
        Err(e) => {
            error!("{e}");
            return ExitCode::FAILURE;
        }
    };
    match options.mode {
        Mode::Resident => return report(resident(store)),
        Mode::Peers => return report(list_peers(&store)),
        Mode::Forget { target } => return report(forget(&store, &target)),
        Mode::Serve { .. } | Mode::Connect { .. } => {}
    }

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
                    announce,
                } => {
                    let remap = if swap_cmd_ctrl {
                        KeyRemap::swap_command_control()
                    } else {
                        KeyRemap::none()
                    };
                    let config = ServerConfig {
                        live: Live { side, remap }.fixed(),
                        name: options.name,
                        clipboard: options.clipboard,
                        store,
                        announce,
                        events: Events::none(),
                    };
                    serve(listen, config).await
                }
                Mode::Connect { server } => {
                    let config = ClientConfig {
                        server: server.map_or(ServerAddr::Discover, ServerAddr::Fixed),
                        name: options.name,
                        clipboard: options.clipboard,
                        store,
                        events: Events::none(),
                    };
                    connect(config).await
                }
                Mode::Resident | Mode::Peers | Mode::Forget { .. } => {
                    unreachable!("已在上面处理")
                }
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
    report(result)
}

/// 常驻运行。已经有一份在运行时，请它打开设置界面，然后退出。
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn resident(store: Store) -> Result<(), Box<dyn std::error::Error>> {
    use ferry_engine::resident::{self, Command};

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        if resident::hand_over_to_running(&store).await {
            info!("ScreenFerry 已在运行，已请它打开设置界面");
            return Ok(());
        }
        let (control, handle) = resident::control();
        // Ctrl+C 走正常退出：停下引擎（归还鼠标、松开按键），删掉 ipc.json。
        tokio::spawn({
            let handle = handle.clone();
            async move {
                if tokio::signal::ctrl_c().await.is_ok() {
                    info!("收到 Ctrl+C，正在退出");
                    handle.send(Command::Quit);
                }
            }
        });
        resident::run(store, crate::host::NativeHost, control).await?;
        drop(handle);
        Ok(())
    })
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn resident(_store: Store) -> Result<(), Box<dyn std::error::Error>> {
    Err("常驻运行目前只支持 macOS 和 Windows".into())
}

fn report(result: Result<(), Box<dyn std::error::Error>>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            error!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn list_peers(store: &Store) -> Result<(), Box<dyn std::error::Error>> {
    let peers = store.peers()?;
    println!("数据目录：{}", store.root().display());
    if peers.is_empty() {
        println!("还没有配对过任何设备。");
    }
    for peer in peers.iter() {
        println!("  {}  {}", peer.fingerprint, peer.name);
    }
    Ok(())
}

/// 按设备名，或者指纹（完整十六进制，或 `peers` 显示的短形式及其前缀）删除一台设备。
fn forget(store: &Store, target: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut peers = store.peers()?;
    let wanted = target.to_uppercase();
    let matches: Vec<Fingerprint> = peers
        .iter()
        .filter(|p| {
            p.name == target
                || p.fingerprint.to_hex().to_uppercase() == wanted
                || (wanted.len() >= 4 && p.fingerprint.short().starts_with(&wanted))
        })
        .map(|p| p.fingerprint)
        .collect();
    match matches.as_slice() {
        [] => Err(format!("没有找到 {target}，可先用 peers 查看").into()),
        [fingerprint] => {
            peers.remove(fingerprint);
            store.save_peers(&peers)?;
            println!("已删除 {fingerprint}，下次连接需要重新配对。");
            Ok(())
        }
        _ => Err(format!("{target} 对应多台设备，请改用更长的指纹").into()),
    }
}

#[cfg(target_os = "macos")]
async fn serve(listen: SocketAddr, config: ServerConfig) -> AppResult {
    use ferry_platform::Permissions;
    use ferry_platform_macos::{MacCapture, MacClipboard, MacDisplays, MacPermissions};

    MacPermissions.check_input()?;
    let listener = tokio::net::TcpListener::bind(listen).await?;
    let (capture, clipboard) = (MacCapture::new(), MacClipboard::new());
    let engine = ferry_engine::serve(
        listener,
        config,
        capture,
        MacDisplays,
        clipboard,
        TerminalConfirm,
    );
    Ok(engine.await?)
}

#[cfg(not(target_os = "macos"))]
async fn serve(_listen: SocketAddr, _config: ServerConfig) -> AppResult {
    Err("原型阶段只能在 macOS 上运行 serve".into())
}

#[cfg(target_os = "windows")]
async fn connect(config: ClientConfig) -> AppResult {
    use ferry_platform::Permissions;
    use ferry_platform_windows::{WinClipboard, WinDisplays, WinInject, WinPermissions};

    WinPermissions.check_input()?;
    let (inject, displays) = (WinInject::new(), WinDisplays::new());
    let engine = ferry_engine::connect(
        config,
        inject,
        displays,
        WinClipboard::new(),
        TerminalConfirm,
    );
    Ok(engine.await?)
}

#[cfg(not(target_os = "windows"))]
async fn connect(_config: ClientConfig) -> AppResult {
    Err("原型阶段只能在 Windows 上运行 connect".into())
}
