use std::time::Duration;

use ferry_engine::{ClipLimit, Event};
use ferry_platform_mock::MockClipboard;
use ferry_types::{ClipboardContent, ClipboardFormat};
use tokio::time::sleep;

use crate::support::{Rig, Setup, collect, eventually, fake_png, has};

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

/// 一份带格式、带图片的内容，像从网页或文档里复制出来的。
fn rich(png_padding: usize) -> ClipboardContent {
    let mut content = text("会议纪要");
    content.insert(ClipboardFormat::HTML, "<b>会议纪要</b>".as_bytes().to_vec());
    content.insert(ClipboardFormat::RTF, br"{\rtf1 x}".to_vec());
    content.insert(ClipboardFormat::PNG, fake_png(640, 480, png_padding));
    content
}

#[tokio::test]
async fn images_and_rich_text_reach_the_other_side() {
    let (mac_events, mac) = collect();
    let rig = Rig::with(Setup {
        mac_events,
        ..Setup::default()
    })
    .await;
    // 约 300 KB 的图片，要分好几块。
    rig.mac_clipboard.copy(rich(300_000));

    rig.enter().await;
    eventually("PC 收到全部格式", || {
        rig.pc_clipboard.content() == rich(300_000)
    })
    .await;
    eventually("Mac 报告图片被拉走了", || {
        has(&mac, |e| {
            matches!(e, Event::Clipboard { to_peer: true, summary, .. }
                if summary.rich && summary.image.as_ref().is_some_and(|i| i.width == 640))
        })
    })
    .await;
}

#[tokio::test]
async fn a_format_over_the_senders_limit_is_skipped_and_reported() {
    let (mac_events, mac) = collect();
    let rig = Rig::with(Setup {
        mac_events,
        mac_clip_limit: ClipLimit::new(100 * 1024),
        ..Setup::default()
    })
    .await;
    rig.mac_clipboard.copy(rich(300_000));

    rig.enter().await;
    eventually("PC 收到文字和格式", || {
        rig.pc_clipboard.writes() == 1
    })
    .await;
    let got = rig.pc_clipboard.content();
    assert_eq!(got.as_text(), Some("会议纪要"));
    assert!(got.get(ClipboardFormat::HTML).is_some());
    assert_eq!(
        got.get(ClipboardFormat::PNG),
        None,
        "图片超过上限，没有发出去"
    );
    assert!(has(&mac, |e| matches!(
        e,
        Event::ClipboardSkipped {
            to_peer: true,
            format: ClipboardFormat::PNG,
            limit: 102_400,
            ..
        }
    )));
}

#[tokio::test]
async fn the_receiver_skips_what_is_over_its_own_limit() {
    let (pc_events, pc) = collect();
    let rig = Rig::with(Setup {
        pc_events,
        pc_clip_limit: ClipLimit::new(100 * 1024),
        ..Setup::default()
    })
    .await;
    rig.mac_clipboard.copy(rich(300_000));

    rig.enter().await;
    eventually("PC 收到文字", || rig.pc_clipboard.writes() == 1).await;
    assert_eq!(rig.pc_clipboard.content().get(ClipboardFormat::PNG), None);
    assert!(has(&pc, |e| matches!(
        e,
        Event::ClipboardSkipped {
            to_peer: false,
            format: ClipboardFormat::PNG,
            ..
        }
    )));
}

#[tokio::test]
async fn raising_the_limit_takes_effect_without_reconnecting() {
    let limit = ClipLimit::new(100 * 1024);
    let rig = Rig::with(Setup {
        mac_clip_limit: limit.clone(),
        ..Setup::default()
    })
    .await;
    rig.mac_clipboard.copy(rich(300_000));
    rig.enter().await;
    eventually("先收到没有图片的那份", || {
        rig.pc_clipboard.writes() == 1
    })
    .await;
    rig.leave().await;

    limit.set(1024 * 1024);
    rig.mac_clipboard.copy(rich(300_001));
    rig.enter().await;
    eventually("调大上限后图片也过来了", || {
        rig.pc_clipboard.content() == rich(300_001)
    })
    .await;
}
