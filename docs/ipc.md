# 本机通信协议（设置界面 ⇄ 后台）

- 当前版本：**1**
- 实现：`crates/ferry-ipc`（Rust，后台）、`app/lib/ipc/`（Dart，设置界面）
- 样例：`app/test/fixtures/ipc_samples.json`，由 `ferry-ipc` 的测试生成，两边的测试都按它编解码
- 决策：ADR 0004

本文是协议的权威说明。

## 1. 找到后台

后台启动时监听 `127.0.0.1` 的随机端口，写下 `ipc.json`：

| 系统 | 位置 |
|---|---|
| macOS | `~/Library/Caches/ScreenFerry/ipc.json` |
| Windows | `%LOCALAPPDATA%\ScreenFerry\ipc.json` |

```json
{ "port": 51234, "token": "64 位十六进制", "pid": 4242, "version": 1 }
```

文件不存在、端口连不上，都表示后台没有运行。后台每次启动都换新端口和新令牌，界面断线后应重新读取这个文件再连。用 `--data-dir` 运行的后台把 `ipc.json` 写在该数据目录里。

## 2. 消息格式

- 每行一条 UTF-8 JSON，以 `\n` 结尾，单行不超过 1 MiB。
- `type` 字段区分消息种类，取值为 snake_case。其余字段与 `type` 平级。
- 时刻一律是 Unix 毫秒（整数），`activity[].hour` 是 Unix 秒。
- 指纹有两种写法：`fingerprint` 是完整的 64 位小写十六进制，用作标识；`short` 是显示用的短形式。
- 接收方忽略不认识的字段。

## 3. 握手

1. 界面发送 `{"type":"hello","token":"…","version":1}`，必须是第一条，3 秒内发出。
2. 令牌不对、版本不同或第一条不是 `hello`，后台直接断开。
3. 后台回 `{"type":"welcome","version":1}`，接着发送 `history`（已有日志，最新的在前）、`state`（当前状态），以及仍在等待的 `pairing`。

## 4. 后台 → 界面

| type | 字段 | 说明 |
|---|---|---|
| `welcome` | `version` | 握手成功 |
| `state` | `state` | 完整状态快照，有变化时推送，最多每秒 4 次。界面整体替换，不做合并 |
| `history` | `entries` | 连接时发送一次 |
| `log` | `entry` | 新日志 |
| `pairing` | `prompt`：`id`、`peer_name`、`fingerprint`、`short`、`code` | 请用户核对配对码 |
| `pairing_closed` | `id` | 这次配对已结束，关闭对话框 |
| `toast` | `text` | 简短提示 |

`state` 的结构（字段含义见 `crates/ferry-ipc/src/message.rs`）：

```text
local     name · fingerprint · short · role(server|client) · data_dir · version · port（服务端监听的端口，否则 null）
sharing   用户没有暂停共享
problem   null | { kind(permission|port_in_use|other), message }
link      status(paused|starting|waiting|searching|connecting|connected|stopped)
          peer(null | { name, fingerprint, short, addr }) · since · cursor_on_peer · rtt_ms[]
clipboard null | { to_peer, peer, chars, bytes, preview, at }
peers[]   { name, fingerprint, short, online, last_seen }
found[]   { name, fingerprint, short, addr, paired }        只有客户端会有
settings  { name, side(left|right|top|bottom), swap_cmd_ctrl, announce, clipboard, server, autostart,
            theme(system|light|dark), accent }
activity[] { hour, switches, clips, connected_secs }         最近 48 小时，只列有活动的小时
```

## 5. 界面 → 后台

| type | 字段 | 说明 |
|---|---|---|
| `hello` | `token`、`version` | 见第 3 节 |
| `set_sharing` | `on` | 暂停或继续共享 |
| `update_settings` | `settings`：`settings` 的任意子集 | 只改出现的字段 |
| `forget` | `fingerprint` | 删除一台已配对设备 |
| `pair_reply` | `id`、`accept` | 回答配对请求 |
| `connect` | `fingerprint` | 客户端：连接 `found` 中的一台服务端，未配对时先配对 |
| `open_permission_settings` | | 打开系统中授予键鼠控制权限的页面 |
| `clear_log` | | 清空日志 |
| `open_settings` | | 请后台打开设置界面（另一份后台用） |

命令没有逐条回执：结果体现在随后的 `state`、`log`、`toast` 里。

## 6. 修改协议

1. 改 `crates/ferry-ipc/src/message.rs` 与本文。
2. 运行 `UPDATE_IPC_SAMPLES=1 cargo test -p ferry-ipc` 重新生成样例。
3. 改 `app/lib/ipc/` 中对应的 Dart 类型，直到 `flutter test` 通过。
4. 只加字段、加消息种类不改版本号；改变已有字段的含义或删除字段时递增 `VERSION`。
