//! 配对确认交给设置界面：引擎问一句，常驻运行转给界面，等用户点按钮。

use tokio::sync::{mpsc, oneshot};

use crate::pairing::{ConfirmPairing, PairingRequest};

/// 一次待回答的配对请求。
pub(crate) struct PairAsk {
    pub(crate) request: PairingRequest,
    pub(crate) reply: oneshot::Sender<bool>,
}

/// 引擎用来请用户确认配对的一端。
#[derive(Clone)]
pub(crate) struct UiConfirm {
    pub(crate) asks: mpsc::UnboundedSender<PairAsk>,
}

impl ConfirmPairing for UiConfirm {
    async fn confirm(&self, request: &PairingRequest) -> bool {
        let (reply, answer) = oneshot::channel();
        let ask = PairAsk {
            request: request.clone(),
            reply,
        };
        if self.asks.send(ask).is_err() {
            return false;
        }
        // 常驻运行退出、或请求被撤销时，按拒绝处理。
        answer.await.unwrap_or(false)
    }
}
