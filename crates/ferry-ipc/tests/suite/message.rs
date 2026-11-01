//! 消息格式。样例同时供设置界面（Dart）的测试使用，两边各自按样例编解码，保证格式一致。
//!
//! 改了消息结构后运行 `UPDATE_IPC_SAMPLES=1 cargo test -p ferry-ipc` 重新生成样例，
//! 再让 `app/` 的测试通过。

use std::path::PathBuf;

use ferry_ipc::{
    ClipboardSync, FoundDevice, HourActivity, Link, LinkStatus, Local, LogEntry, LogIcon, LogKind,
    Notice, PairingPrompt, PeerInfo, PeerRef, Problem, ProblemKind, Request, Settings,
    SettingsPatch, State, VERSION,
};
use ferry_types::{Role, Side, Theme};
use serde_json::{Value, json};

const FP_PC: &str = "3f9a12c477e0b1d2aa55aa55aa55aa55aa55aa55aa55aa55aa55aa55aa55aa55";
const FP_MAC: &str = "7ab05cb238fbd60311223344556677889900aabbccddeeff0011223344556677";

fn samples_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../app/test/fixtures/ipc_samples.json")
}

fn requests() -> Vec<Request> {
    vec![
        Request::Hello {
            token: "ab".repeat(32),
            version: VERSION,
        },
        Request::SetSharing { on: false },
        Request::UpdateSettings {
            settings: SettingsPatch {
                side: Some(Side::Left),
                autostart: Some(true),
                theme: Some(Theme::Light),
                ..SettingsPatch::default()
            },
        },
        Request::Forget {
            fingerprint: FP_PC.into(),
        },
        Request::PairReply {
            id: 3,
            accept: true,
        },
        Request::Connect {
            fingerprint: FP_MAC.into(),
        },
        Request::OpenPermissionSettings,
        Request::ClearLog,
        Request::OpenSettings,
        Request::SendFiles {
            paths: vec![
                "/Users/me/Desktop/证书.p12".into(),
                "/Users/me/Desktop/设计稿".into(),
            ],
        },
    ]
}

fn state() -> State {
    State {
        local: Local {
            name: "Mac Studio".into(),
            fingerprint: FP_MAC.into(),
            short: "7AB0-5CB2-38FB-D603".into(),
            role: Role::Server,
            data_dir: "/Users/me/Library/Application Support/ScreenFerry".into(),
            version: "0.1.0".into(),
            port: Some(24810),
        },
        sharing: true,
        problem: None,
        link: Link {
            status: LinkStatus::Connected,
            peer: Some(PeerRef {
                name: "办公室 PC".into(),
                fingerprint: FP_PC.into(),
                short: "3F9A-12C4-77E0-B1D2".into(),
                addr: "192.168.2.18".into(),
            }),
            since: Some(1_793_480_000_000),
            cursor_on_peer: true,
            rtt_ms: vec![0.31, 0.28, 0.5],
        },
        clipboard: Some(ClipboardSync {
            to_peer: true,
            peer: "办公室 PC".into(),
            chars: Some(31),
            bytes: 93,
            preview: "周三例会纪要".into(),
            at: 1_793_480_100_000,
        }),
        peers: vec![PeerInfo {
            name: "办公室 PC".into(),
            fingerprint: FP_PC.into(),
            short: "3F9A-12C4-77E0-B1D2".into(),
            online: true,
            last_seen: None,
        }],
        found: vec![FoundDevice {
            name: "会议室 Mac".into(),
            fingerprint: FP_MAC.into(),
            short: "7AB0-5CB2-38FB-D603".into(),
            addr: "192.168.2.31".into(),
            paired: false,
        }],
        settings: Settings {
            name: String::new(),
            side: Side::Right,
            swap_cmd_ctrl: true,
            announce: true,
            clipboard: true,
            server: String::new(),
            autostart: false,
            theme: Theme::Dark,
            accent: 2,
        },
        activity: vec![HourActivity {
            hour: 1_793_476_800,
            switches: 12,
            clips: 3,
            connected_secs: 3600,
        }],
    }
}

fn notices() -> Vec<Notice> {
    let entry = LogEntry {
        id: 9,
        kind: LogKind::Cursor,
        icon: LogIcon::Cursor,
        text: "光标切到 办公室 PC".into(),
        detail: "从本机右侧边缘".into(),
        at: 1_793_480_200_000,
    };
    let stopped = State {
        sharing: false,
        problem: Some(Problem {
            kind: ProblemKind::Permission,
            message: "需要在“隐私与安全性”中允许 ScreenFerry 控制电脑".into(),
        }),
        link: Link {
            status: LinkStatus::Stopped,
            peer: None,
            since: None,
            cursor_on_peer: false,
            rtt_ms: vec![],
        },
        clipboard: None,
        ..state()
    };
    vec![
        Notice::Welcome { version: VERSION },
        Notice::State {
            state: Box::new(state()),
        },
        Notice::State {
            state: Box::new(stopped),
        },
        Notice::History {
            entries: vec![entry.clone()],
        },
        Notice::Log { entry },
        Notice::Pairing {
            prompt: PairingPrompt {
                id: 3,
                peer_name: "会议室 PC".into(),
                fingerprint: FP_PC.into(),
                short: "3F9A-12C4-77E0-B1D2".into(),
                code: "482 913".into(),
            },
        },
        Notice::PairingClosed { id: 3 },
        Notice::Toast {
            text: "已暂停共享".into(),
        },
    ]
}

#[test]
fn every_sample_round_trips() {
    for request in requests() {
        let json = serde_json::to_string(&request).unwrap();
        assert_eq!(
            serde_json::from_str::<Request>(&json).unwrap(),
            request,
            "{json}"
        );
    }
    for notice in notices() {
        let json = serde_json::to_string(&notice).unwrap();
        assert_eq!(
            serde_json::from_str::<Notice>(&json).unwrap(),
            notice,
            "{json}"
        );
    }
}

#[test]
fn wire_shape_is_flat_and_snake_case() {
    assert_eq!(
        serde_json::to_value(Request::SetSharing { on: true }).unwrap(),
        json!({"type": "set_sharing", "on": true})
    );
    assert_eq!(
        serde_json::to_value(Request::OpenPermissionSettings).unwrap(),
        json!({"type": "open_permission_settings"})
    );
    // 补丁里没改的字段不出现。
    assert_eq!(
        serde_json::to_value(SettingsPatch {
            clipboard: Some(false),
            ..SettingsPatch::default()
        })
        .unwrap(),
        json!({"clipboard": false})
    );
}

#[test]
fn unknown_patch_fields_and_nulls_are_ignored() {
    let patch: SettingsPatch =
        serde_json::from_value(json!({"side": "top", "clipboard": null, "later": 1})).unwrap();
    assert_eq!(patch.side, Some(Side::Top));
    assert_eq!(patch.clipboard, None);
}

/// 与界面共用的样例文件必须与这里的定义一致。
#[test]
fn samples_shared_with_the_settings_ui_are_up_to_date() {
    let expected = json!({
        "version": VERSION,
        "requests": requests(),
        "notices": notices(),
    });
    let path = samples_path();
    if std::env::var_os("UPDATE_IPC_SAMPLES").is_some() {
        let text = serde_json::to_string_pretty(&expected).unwrap() + "\n";
        std::fs::write(&path, text).unwrap();
        return;
    }
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let actual: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    assert!(
        actual == expected,
        "{} 已过期：运行 UPDATE_IPC_SAMPLES=1 cargo test -p ferry-ipc 重新生成，再更新 app/ 中的 Dart 类型",
        path.display()
    );
}
