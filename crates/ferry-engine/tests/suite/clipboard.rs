use std::time::Duration;

use ferry_platform_mock::MockClipboard;
use ferry_types::ClipboardContent;
use tokio::time::sleep;

use crate::support::{Rig, eventually};

/// 比轮询间隔（250 ms）长得多，足够让不该发生的同步发生。
const QUIET: Duration = Duration::from_millis(700);

fn text(t: &str) -> ClipboardContent {
    ClipboardContent::text(t)
}

#[tokio::test]
async fn copying_on_the_mac_reaches_the_pc_when_the_cursor_goes_there() {
    let rig = Rig::start().await;
    rig.mac_clipboard.copy(text("来自 Mac"));

    sleep(QUIET).await;
    assert_eq!(
        rig.pc_clipboard.writes(),
        0,
        "光标没过去，不该写入对方剪贴板"
    );

    rig.enter().await;
    eventually("PC 收到文本", || {
        rig.pc_clipboard.content() == text("来自 Mac")
    })
    .await;
}

#[tokio::test]
async fn copying_on_the_pc_reaches_the_mac_when_the_cursor_returns() {
    let rig = Rig::start().await;
    rig.enter().await;
    rig.pc_clipboard.copy(text("来自 PC"));

    rig.leave().await;
    eventually("Mac 收到文本", || {
        rig.mac_clipboard.content() == text("来自 PC")
    })
    .await;
}

#[tokio::test]
async fn only_the_latest_copy_is_transferred() {
    let rig = Rig::start().await;
    for i in 0..3 {
        rig.mac_clipboard.copy(text(&format!("第 {i} 次")));
        sleep(Duration::from_millis(300)).await;
    }

    rig.enter().await;
    eventually("PC 收到最后一次", || {
        rig.pc_clipboard.content() == text("第 2 次")
    })
    .await;
    sleep(QUIET).await;
    assert_eq!(rig.pc_clipboard.writes(), 1);
}

#[tokio::test]
async fn synced_content_is_not_echoed_back() {
    let rig = Rig::start().await;
    rig.mac_clipboard.copy(text("回声"));
    rig.enter().await;
    eventually("PC 收到文本", || rig.pc_clipboard.writes() == 1).await;

    rig.leave().await;
    sleep(QUIET).await;
    rig.enter().await;
    sleep(QUIET).await;

    assert_eq!(rig.mac_clipboard.writes(), 0, "Mac 不该收到自己的内容");
    assert_eq!(rig.pc_clipboard.writes(), 1, "同一份内容只写一次");
}

#[tokio::test]
async fn connecting_does_not_overwrite_the_mac_with_old_pc_content() {
    let mac = MockClipboard::new();
    let pc = MockClipboard::new();
    mac.copy(text("Mac 上的"));
    pc.copy(text("PC 上旧的"));
    let rig = Rig::with_clipboards(mac, pc).await;

    sleep(QUIET).await;
    rig.enter().await;
    eventually("PC 收到 Mac 的内容", || {
        rig.pc_clipboard.content() == text("Mac 上的")
    })
    .await;
    rig.leave().await;
    sleep(QUIET).await;

    assert_eq!(rig.mac_clipboard.content(), text("Mac 上的"));
    assert_eq!(rig.mac_clipboard.writes(), 0);
}

#[tokio::test]
async fn crossing_right_after_connecting_still_keeps_each_side_right() {
    // 数据连接比输入连接晚一点建立，这期间光标就已经切过去了。
    let mac = MockClipboard::new();
    let pc = MockClipboard::new();
    mac.copy(text("Mac 上的"));
    pc.copy(text("PC 上旧的"));
    let rig = Rig::with_clipboards(mac, pc).await;

    rig.enter().await;
    eventually("PC 收到 Mac 的内容", || {
        rig.pc_clipboard.content() == text("Mac 上的")
    })
    .await;
    rig.leave().await;
    sleep(QUIET).await;
    assert_eq!(rig.mac_clipboard.writes(), 0, "PC 的旧内容不能覆盖 Mac");
}

#[tokio::test]
async fn large_text_arrives_in_many_chunks() {
    let rig = Rig::start().await;
    // 约 600 KB，需要十来个 64 KiB 分块。
    let big = "剪贴板分块测试。\n".repeat(25_000);
    rig.mac_clipboard.copy(text(&big));

    rig.enter().await;
    eventually("PC 收到完整文本", || rig.pc_clipboard.writes() == 1).await;
    assert_eq!(rig.pc_clipboard.content().as_text(), Some(big.as_str()));
}

#[tokio::test]
async fn copying_something_unsupported_clears_the_pending_offer() {
    let rig = Rig::start().await;
    rig.mac_clipboard.copy(text("旧文本"));
    sleep(Duration::from_millis(300)).await;
    // 比如复制了一张图片：本机读不出任何支持的格式。
    rig.mac_clipboard.copy(ClipboardContent::new());
    sleep(Duration::from_millis(300)).await;

    rig.enter().await;
    sleep(QUIET).await;
    assert_eq!(rig.pc_clipboard.writes(), 0, "不该粘贴出之前的旧文本");
}
