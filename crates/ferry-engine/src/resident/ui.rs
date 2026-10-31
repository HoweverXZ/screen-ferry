//! 设置界面的连接：握手放到后台，之后一个任务收命令、一个任务发通知。

use ferry_ipc::{Incoming, Notice, Request};
use tokio::sync::mpsc;
use tracing::{debug, info};

/// 发给一个界面的通知最多积压的条数。积压满了说明界面卡住了，断开它，等它重连后重新同步。
const OUTBOX: usize = 256;

/// 界面连接上发生的事。
pub(crate) enum UiEvent {
    /// 新界面握手成功。`outbox` 用来给它发通知。
    Joined {
        id: u64,
        outbox: mpsc::Sender<Notice>,
    },
    Request(Request),
    Left {
        id: u64,
    },
}

/// 在后台核对令牌，成功后开始收发。
pub(crate) fn accept(incoming: Incoming, id: u64, events: mpsc::UnboundedSender<UiEvent>) {
    tokio::spawn(async move {
        let conn = match incoming.authenticate().await {
            Ok(conn) => conn,
            Err(e) => {
                debug!("拒绝了一个本机连接：{e}");
                return;
            }
        };
        info!(id, "设置界面已连接");
        let (mut reader, mut writer) = conn.split();
        let (outbox, mut queue) = mpsc::channel::<Notice>(OUTBOX);
        if events.send(UiEvent::Joined { id, outbox }).is_err() {
            return;
        }
        let send = async {
            while let Some(notice) = queue.recv().await {
                if writer.send(&notice).await.is_err() {
                    break;
                }
            }
        };
        let receive = async {
            loop {
                match reader.recv::<Request>().await {
                    Ok(Some(request)) => {
                        if events.send(UiEvent::Request(request)).is_err() {
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        debug!(id, "设置界面发来的消息有误：{e}");
                        break;
                    }
                }
            }
        };
        tokio::select! {
            () = send => {}
            () = receive => {}
        }
        info!(id, "设置界面已断开");
        let _ = events.send(UiEvent::Left { id });
    });
}
