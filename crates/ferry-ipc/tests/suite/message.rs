//! 消息格式。样例同时供设置界面（Dart）的测试使用，两边各自按样例编解码，保证格式一致。
//!
//! 改了消息结构后运行 `UPDATE_IPC_SAMPLES=1 cargo test -p ferry-ipc` 重新生成样例，
//! 再让 `app/` 的测试通过。

use std::path::PathBuf;

use ferry_ipc::{
    ClipboardImage, ClipboardSync, DialError, FoundDevice, HourActivity, Link, LinkStatus, Local,
    LogEntry, LogIcon, LogKind, Notice, PairingPrompt, PeerInfo, PeerRef, Phrase, Problem,
    ProblemKind, Request, RunLogLevel, RunLogLine, Settings, SettingsPatch, State, UpdateError,
    UpdateInfo, UpdateRelease, UpdateStatus, VERSION,
};
use ferry_types::{
    DeviceSettings, KeyMapping, Language, ModifierPreset, ModifierTarget, Placement, Role,
    ScrollSpeed, Side, Theme,
};
use serde_json::{Value, json};

const FP_PC: &str = "3f9a12c477e0b1d2aa55aa55aa55aa55aa55aa55aa55aa55aa55aa55aa55aa55";
const FP_MAC: &str = "7ab05cb238fbd60311223344556677889900aabbccddeeff0011223344556677";

fn samples_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../app/test/fixtures/ipc_samples.json")
}

/// 一台单独设置过的设备：每个字段都不是缺省值，两边的解码都要覆盖到。
fn device() -> DeviceSettings {
    let mut d = DeviceSettings::new(FP_PC);
    d.alias = "书房的电脑".into();
    d.pointer_dpi = 2000;
    d.reverse_vertical = true;
    d.reverse_horizontal = true;
    d.scroll_speed = ScrollSpeed::Fast;
    d.modifiers = ModifierPreset::Custom;
    d.text_shortcuts = true;
    d.edge_delay_ms = 200;
    d.clipboard = false;
    d.files = false;
    d.custom_modifiers.caps_lock = ModifierTarget::Ctrl;
    d.custom_modifiers.alt = ModifierTarget::None;
    d.keys = vec![
        // F13 → Print Screen；Caps Lock 不发送。
        KeyMapping {
            from: 0x0007_0068,
            to: 0x0007_0046,
        },
        KeyMapping {
            from: 0x0007_0039,
            to: 0,
        },
    ];
    d
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
                language: Some(Language::Zh),
                auto_update: Some(false),
                clipboard_limit_mb: Some(16),
                mouse_dpi: Some(800),
                ..SettingsPatch::default()
            },
        },
        Request::UpdateSettings {
            settings: SettingsPatch {
                layout: Some(vec![
                    Placement {
                        fingerprint: FP_PC.into(),
                        side: Side::Left,
                    },
                    Placement {
                        fingerprint: FP_MAC.into(),
                        side: Side::Right,
                    },
                ]),
                ..SettingsPatch::default()
            },
        },
        Request::UpdateSettings {
            settings: SettingsPatch {
                role: Some(Role::Client),
                ..SettingsPatch::default()
            },
        },
        Request::SetDevice { device: device() },
        Request::SetDevice {
            device: DeviceSettings::new(FP_MAC),
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
        Request::ConnectAddress {
            address: "192.168.2.12".into(),
        },
        Request::Invite {
            address: "192.168.2.18:24811".into(),
        },
        Request::OpenPermissionSettings,
        Request::ClearLog,
        Request::OpenSettings,
        Request::OpenPeerSettings {
            fingerprint: FP_PC.into(),
        },
        Request::SendFiles {
            paths: vec![
                "/Users/me/Desktop/证书.p12".into(),
                "/Users/me/Desktop/设计稿".into(),
            ],
            to: None,
        },
        Request::SendFiles {
            paths: vec!["/Users/me/Desktop/证书.p12".into()],
            to: Some(FP_PC.into()),
        },
        Request::Quit,
        Request::ReadRunLog { since: None },
        Request::ReadRunLog {
            since: Some(204_800),
        },
        Request::ExportDiagnostics,
        Request::CopyDiagnostics,
        Request::RevealDiagnostics,
        Request::OpenLogsFolder,
        Request::SetVerbose { on: true },
        Request::CheckUpdate,
        Request::DownloadUpdate,
        Request::CancelUpdate,
        Request::InstallUpdate,
        Request::DismissUpdate {
            version: "0.2.0".into(),
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
            platform: "macos".into(),
            natural_scrolling: Some(true),
            addresses: vec!["192.168.2.12".into(), "10.8.0.3".into()],
            invite_port: None,
            system: "macOS 15.6 · arm64".into(),
            verbose_since: Some(1_793_480_000_000),
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
            rich: true,
            image: Some(ClipboardImage {
                width: 1920,
                height: 1080,
                path: Some(
                    "/Users/me/Library/Application Support/ScreenFerry/blobs/ab/cdef".into(),
                ),
            }),
            at: 1_793_480_100_000,
        }),
        peers: vec![PeerInfo {
            name: "办公室 PC".into(),
            fingerprint: FP_PC.into(),
            short: "3F9A-12C4-77E0-B1D2".into(),
            online: true,
            last_seen: None,
            side: Some(Side::Left),
            cursor: true,
            alias: "书房的电脑".into(),
            platform: "windows".into(),
            addr: "192.168.2.18".into(),
            version: "0.1.0".into(),
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
            layout: vec![Placement {
                fingerprint: FP_PC.into(),
                side: Side::Left,
            }],
            announce: true,
            clipboard: true,
            clipboard_limit_mb: 32,
            mouse_dpi: 1600,
            server: String::new(),
            autostart: false,
            theme: Theme::Dark,
            accent: 2,
            language: Language::En,
            auto_update: true,
            devices: vec![device()],
        },
        activity: vec![HourActivity {
            hour: 1_793_476_800,
            switches: 12,
            clips: 3,
            connected_secs: 3600,
        }],
        update: UpdateInfo {
            status: UpdateStatus::Downloading,
            latest: Some(UpdateRelease {
                version: "0.2.0".into(),
                published: 1_795_161_600_000,
                size: 21_000_000,
                notes_zh: "英文界面\n运行日志与诊断导出".into(),
                notes_en: "English UI\nDiagnostics log and export".into(),
                breaks_peers: false,
            }),
            done: 13_000_000,
            total: 21_000_000,
            error: None,
            message: String::new(),
            checked_at: Some(1_795_165_200_000),
            can_install: true,
            needs_local_confirm: false,
            dismissed: String::new(),
        },
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
        phrase: Some(
            Phrase::new("cursor_to_peer")
                .arg("name", "办公室 PC")
                .arg("side", "right"),
        ),
    };
    let stopped = State {
        sharing: false,
        problem: Some(Problem {
            kind: ProblemKind::Permission,
            message: "需要在“隐私与安全性”中允许 ScreenFerry 控制电脑".into(),
            port: None,
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
            phrase: Some(Phrase::new("sharing_paused")),
        },
        Notice::ShowDevice {
            fingerprint: FP_PC.into(),
        },
        Notice::Dialed {
            address: "192.168.2.12".into(),
            invite: false,
            error: None,
        },
        Notice::Dialed {
            address: "192.168.2.18".into(),
            invite: true,
            error: Some(DialError::NotScreenFerry),
        },
        Notice::Quit,
        Notice::RunLog {
            lines: vec![
                RunLogLine {
                    at: "06:12:04".into(),
                    level: RunLogLevel::Info,
                    text: "已连接 Windows · TLS 1.3 · 192.168.2.18".into(),
                },
                RunLogLine {
                    at: "06:20:02".into(),
                    level: RunLogLevel::Warn,
                    text: "连接中断：网络错误".into(),
                },
            ],
            next: 204_800,
            fresh: true,
            bytes: 2_097_152,
            lines_total: 18_241,
        },
        Notice::DiagnosticsExported {
            path: "/Users/me/Downloads/ScreenFerry-诊断-20261102-0624.txt".into(),
            bytes: 421_888,
        },
        Notice::DiagnosticsSummary {
            text: "ScreenFerry 诊断摘要\n版本 0.1.0".into(),
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
fn an_old_daemon_without_update_fields_still_parses() {
    let mut value = serde_json::to_value(state()).unwrap();
    value.as_object_mut().unwrap().remove("update");
    value["settings"]
        .as_object_mut()
        .unwrap()
        .remove("auto_update");
    value["peers"][0].as_object_mut().unwrap().remove("version");
    let parsed: State = serde_json::from_value(value).unwrap();
    assert_eq!(parsed.update, UpdateInfo::default());
    assert!(parsed.settings.auto_update, "缺省开着");
    assert_eq!(
        serde_json::to_value(UpdateError::VerifyFailed).unwrap(),
        json!("verify_failed")
    );
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
