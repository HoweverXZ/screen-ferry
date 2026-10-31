//! 命令行参数。

use std::net::{IpAddr, SocketAddr, ToSocketAddrs};

use ferry_core::Side;
use ferry_engine::DEFAULT_PORT;

pub(crate) const USAGE: &str = "\
用法：
  screenferry serve   [--listen 地址] [--side 方向] [--name 设备名] [--no-cmd-ctrl-swap] [--no-clipboard] [-v]
  screenferry connect <服务端地址> [--name 设备名] [--no-clipboard] [-v]

命令：
  serve      在拥有键鼠的电脑上运行，等待另一台连接
  connect    在被控制的电脑上运行，连接到运行 serve 的那台

选项：
  --listen   监听地址，默认 0.0.0.0:24810
  --side     对方屏幕在本机的哪一侧：right、left、top、bottom，默认 right
  --name     显示给对方的设备名，默认取计算机名
  --no-cmd-ctrl-swap
             不互换 Command 与 Ctrl。默认互换，在 Windows 上 ⌘C、⌘V 即复制粘贴
  --no-clipboard
             不同步剪贴板。任意一端加上即关闭
  -v         输出调试日志
  -h         显示本帮助

光标在对方屏幕时，按 Ctrl + Option + Esc 可以强制回到本机。";

pub(crate) enum Parsed {
    Run(Options),
    Help,
}

pub(crate) struct Options {
    pub(crate) mode: Mode,
    pub(crate) name: String,
    pub(crate) clipboard: bool,
    pub(crate) verbose: bool,
}

pub(crate) enum Mode {
    Serve {
        listen: SocketAddr,
        side: Side,
        swap_cmd_ctrl: bool,
    },
    Connect {
        server: SocketAddr,
    },
}

pub(crate) fn parse(args: impl IntoIterator<Item = String>) -> Result<Parsed, String> {
    let mut args = args.into_iter();
    let command = match args.next().as_deref() {
        None | Some("-h" | "--help" | "help") => return Ok(Parsed::Help),
        Some(command @ ("serve" | "connect")) => command.to_owned(),
        Some(other) => return Err(format!("未知命令：{other}")),
    };

    let mut listen = SocketAddr::from(([0, 0, 0, 0], DEFAULT_PORT));
    let mut side = Side::Right;
    let mut swap_cmd_ctrl = true;
    let mut server = None;
    let mut name = None;
    let mut clipboard = true;
    let mut verbose = false;

    while let Some(arg) = args.next() {
        let mut value = |flag: &str| args.next().ok_or_else(|| format!("{flag} 缺少取值"));
        match arg.as_str() {
            "-h" | "--help" => return Ok(Parsed::Help),
            "-v" | "--verbose" => verbose = true,
            "--name" => name = Some(value("--name")?),
            "--no-clipboard" => clipboard = false,
            "--listen" if command == "serve" => listen = resolve(&value("--listen")?)?,
            "--side" if command == "serve" => side = parse_side(&value("--side")?)?,
            "--no-cmd-ctrl-swap" if command == "serve" => swap_cmd_ctrl = false,
            other if command == "connect" && server.is_none() && !other.starts_with('-') => {
                server = Some(resolve(other)?);
            }
            other => return Err(format!("无法识别的参数：{other}")),
        }
    }

    let mode = match command.as_str() {
        "serve" => Mode::Serve {
            listen,
            side,
            swap_cmd_ctrl,
        },
        _ => Mode::Connect {
            server: server.ok_or("connect 需要服务端地址，例如 192.168.1.5")?,
        },
    };
    Ok(Parsed::Run(Options {
        mode,
        name: name.unwrap_or_else(default_name),
        clipboard,
        verbose,
    }))
}

/// 解析 `IP`、`IP:端口`、`主机名` 或 `主机名:端口`，省略端口时使用默认端口。
fn resolve(text: &str) -> Result<SocketAddr, String> {
    if let Ok(addr) = text.parse::<SocketAddr>() {
        return Ok(addr);
    }
    if let Ok(ip) = text.parse::<IpAddr>() {
        return Ok(SocketAddr::new(ip, DEFAULT_PORT));
    }
    let with_port = if text.contains(':') {
        text.to_owned()
    } else {
        format!("{text}:{DEFAULT_PORT}")
    };
    with_port
        .to_socket_addrs()
        .ok()
        .and_then(|mut addrs| addrs.next())
        .ok_or_else(|| format!("无法解析地址：{text}"))
}

fn parse_side(text: &str) -> Result<Side, String> {
    match text {
        "right" => Ok(Side::Right),
        "left" => Ok(Side::Left),
        "top" => Ok(Side::Top),
        "bottom" => Ok(Side::Bottom),
        _ => Err(format!(
            "--side 只能是 right、left、top、bottom，而不是 {text}"
        )),
    }
}

fn default_name() -> String {
    std::env::var("COMPUTERNAME")
        .ok()
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| {
            if cfg!(target_os = "macos") {
                "Mac".into()
            } else {
                "ScreenFerry".into()
            }
        })
}
