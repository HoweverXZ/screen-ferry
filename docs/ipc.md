# 本机通信协议（设置界面 ⇄ 后台）

- 当前版本：**2**（版本 2 去掉了 `settings.swap_cmd_ctrl`，改为每台设备的 `settings.devices[].modifiers`）
- 实现：`crates/ferry-ipc`（Rust，后台）、`app/lib/ipc/`（Dart，设置界面）
- 样例：`app/test/fixtures/ipc_samples.json`，由 `ferry-ipc` 的测试生成，两边的测试都按它编解码
- 决策：ADR 0004；每台设备的单独设置见 ADR 0009

本文是协议的权威说明。

## 1. 找到后台

后台启动时监听 `127.0.0.1` 的随机端口，写下 `ipc.json`：

| 系统 | 位置 |
|---|---|
| macOS | `~/Library/Caches/ScreenFerry/ipc.json` |
| Windows | `%LOCALAPPDATA%\ScreenFerry\ipc.json` |

```json
{ "port": 51234, "token": "64 位十六进制", "pid": 4242, "version": 2 }
```

文件不存在、端口连不上，都表示后台没有运行。后台每次启动都换新端口和新令牌，界面断线后应重新读取这个文件再连。用 `--data-dir` 运行的后台把 `ipc.json` 写在该数据目录里。

## 2. 消息格式

- 每行一条 UTF-8 JSON，以 `\n` 结尾，单行不超过 1 MiB。
- `type` 字段区分消息种类，取值为 snake_case。其余字段与 `type` 平级。
- 时刻一律是 Unix 毫秒（整数），`activity[].hour` 是 Unix 秒。
- 指纹有两种写法：`fingerprint` 是完整的 64 位小写十六进制，用作标识；`short` 是显示用的短形式。
- 接收方忽略不认识的字段。

## 3. 握手

1. 界面发送 `{"type":"hello","token":"…","version":2}`，必须是第一条，3 秒内发出。
2. 令牌不对、版本不同或第一条不是 `hello`，后台直接断开。
3. 后台回 `{"type":"welcome","version":2}`，接着发送 `history`（已有日志，最新的在前）、`state`（当前状态），以及仍在等待的 `pairing`。

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
| `show_device` | `fingerprint` | 翻到这台设备的设置页：它请本机打开了设置（本机是服务端、它被本机控制时） |
| `dialed` | `address`、`invite`、`error` | 一次 `connect_address`（`invite` 为 false）或 `invite`（为 true）的结果。`error` 为 null 表示成了：按地址连接是已经连上（没配对时接着发 `pairing`），邀请是对方答应了、正在连过来。否则是 `bad_address`（地址不对）、`refused`（那里没有在等的 ScreenFerry）、`unreachable`（连不上）、`not_screen_ferry`、`incompatible`（版本不兼容）、`busy`（对方正连着别的电脑）、`wrong_role`（本机的角色做不了） |
| `quit` | | 后台要退出了（托盘里的“退出”或 `quit` 命令），界面随之关闭。之后连接断开 |

`state` 的结构（字段含义见 `crates/ferry-ipc/src/message.rs`）：

```text
local     name · fingerprint · short · role(server|client) · data_dir · version · port（服务端监听的端口，否则 null）
          platform（本机系统，macos|windows…；旧版后台没有，为空）
          natural_scrolling（macOS：开着“自然滚动”；其他系统为 null）
          addresses[]（本机的局域网 IPv4 地址，常用的在前）· invite_port（客户端等邀请的端口，否则 null）
sharing   用户没有暂停共享
problem   null | { kind(permission|port_in_use|other), message }
link      status(paused|starting|waiting|searching|connecting|connected|stopped)
          peer(null | { name, fingerprint, short, addr }) · since · cursor_on_peer · rtt_ms[]
          连着几台时是摘要：指光标所在（否则最近连上）的那台；每台是否连着见 peers
clipboard null | { to_peer, peer, chars, bytes, preview, rich（带格式）, at,
                 image（null | { width, height, path（预览用的 PNG 文件，存不下来时为 null）}） }
peers[]   { name（它自己报的名字）, fingerprint, short, online, last_seen,
            side（服务端：在本机哪一侧；客户端为 null）, cursor（光标正在它上面）,
            alias（本机给它起的名字，空为没起）, platform（macos|windows，本次运行还没看出来时为空）,
            addr（上次连上的地址，没有为空；端口是缺省值时省略） }
found[]   { name, fingerprint, short, addr, paired }        只有客户端会有
settings  { name, side(left|right|top|bottom), layout[], announce, clipboard,
            clipboard_limit_mb（剪贴板单项上限，1–64）, mouse_dpi（本机鼠标的 DPI，100–12800）,
            server（客户端：只连这个地址，空为自动寻找）, autostart, theme(system|light|dark), accent, devices[] }
          layout[] { fingerprint, side }：单独安排过位置的设备，按先后排列，同一侧的几台按这个先后
          平分那条边（左右两侧从上到下，上下两侧从左到右）；side 是没有列出的设备放的那一侧
          devices[] 单独设置过的设备，没列出的用缺省值（括号里）：
            { fingerprint, alias（""）, pointer_dpi（0：和本机一样）, reverse_vertical（false）,
              reverse_horizontal（false）, scroll_speed(slow|normal|fast)（normal）,
              modifiers(habit|plain|custom)（habit）, text_shortcuts（false）, edge_delay_ms（0）,
              clipboard（true）, files（true）,
              custom_modifiers { gui, ctrl, alt, caps_lock }：各为 ctrl|shift|alt|gui|caps_lock|none,
              keys[] { from, to }：USB HID 扩展用法码，to 为 0 表示不发送 }
            鼠标、按键、防误切只在本机控制它（本机是服务端）时生效；剪贴板、拖文件两边都生效
activity[] { hour, switches, clips, connected_secs }         最近 48 小时，只列有活动的小时
```

## 5. 界面 → 后台

| type | 字段 | 说明 |
|---|---|---|
| `hello` | `token`、`version` | 见第 3 节 |
| `set_sharing` | `on` | 暂停或继续共享 |
| `update_settings` | `settings`：`settings` 的任意子集，另可带 `role`（server\|client） | 只改出现的字段；`layout` 整份替换；改 `role` 会断开现有的连接、以新角色重新开始 |
| `set_device` | `device`：`settings.devices[]` 的一项，整份 | 换上一台设备的单独设置；全是缺省值即恢复默认 |
| `open_peer_settings` | `fingerprint` | 客户端：请连着的服务端打开它的设置界面、翻到本机的设置页，结果以 `toast` 告知 |
| `forget` | `fingerprint` | 删除一台已配对设备，连同它的单独设置 |
| `pair_reply` | `id`、`accept` | 回答配对请求 |
| `connect` | `fingerprint` | 客户端：连接 `found` 中的一台服务端，未配对时先配对 |
| `connect_address` | `address`：IP、IP:端口或电脑名，不写端口用 24810 | 客户端：连接这个地址上的服务端，未配对时先配对，结果以 `dialed` 告知。只连这一次，不改 `settings.server`；连上后断线会回到这个地址重连 |
| `invite` | `address`：同上，不写端口用 24811 | 服务端：请这个地址上的客户端连过来，结果以 `dialed` 告知 |
| `open_permission_settings` | | 打开系统中授予键鼠控制权限的页面 |
| `clear_log` | | 清空日志 |
| `open_settings` | | 请后台打开设置界面（另一份后台用） |
| `quit` | | 请后台退出：停下共享（归还键鼠、松开按键），给所有界面发 `quit` 后退出。`screenferry quit` 用，安装程序升级、卸载前靠它放开程序文件 |
| `send_files` | `paths`：本机绝对路径的列表；可选 `to`：目标设备的证书指纹 | 把这些文件和文件夹发给 `to`，不填时发给光标最近去过的那台（只连着一台时就是它；`screenferry send` 用），结果以 `toast` 和日志告知 |

除了 `connect_address` 和 `invite` 以 `dialed` 回复，命令没有逐条回执：结果体现在随后的 `state`、`log`、`toast` 里。

## 6. 修改协议

1. 改 `crates/ferry-ipc/src/message.rs` 与本文。
2. 运行 `UPDATE_IPC_SAMPLES=1 cargo test -p ferry-ipc` 重新生成样例。
3. 改 `app/lib/ipc/` 中对应的 Dart 类型，直到 `flutter test` 通过。
4. 只加字段、加消息种类不改版本号；改变已有字段的含义或删除字段时递增 `VERSION`。
