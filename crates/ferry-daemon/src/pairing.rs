//! 在终端里请用户核对配对码。

use std::io::Write;

use ferry_engine::{ConfirmPairing, PairingRequest};
use tokio::io::{AsyncBufReadExt, BufReader};

/// 把配对请求打印到终端，等用户输入 y 确认。
pub(crate) struct TerminalConfirm;

impl ConfirmPairing for TerminalConfirm {
    async fn confirm(&self, request: &PairingRequest) -> bool {
        let prompt = format!(
            "\n\
             ──────────── 配对请求 ────────────\n\
             \x20 对方设备：{}\n\
             \x20 对方指纹：{}\n\
             \x20 配对码：  {}\n\
             ──────────────────────────────────\n\
             请确认对方屏幕上显示的是同一个配对码。\n\
             一致请输入 y 并回车；不一致或不认识这台设备，直接回车拒绝：",
            request.peer_name, request.peer_fingerprint, request.code,
        );
        {
            let mut err = std::io::stderr().lock();
            let _ = writeln!(err, "{prompt}");
            let _ = err.flush();
        }

        let mut line = String::new();
        let mut stdin = BufReader::new(tokio::io::stdin());
        match stdin.read_line(&mut line).await {
            // 没有终端（例如后台运行）时读到文件末尾，按拒绝处理。
            Ok(0) | Err(_) => false,
            Ok(_) => matches!(line.trim().to_lowercase().as_str(), "y" | "yes" | "是"),
        }
    }
}
