# 线协议

- 当前版本：**1**（草案：首次发布前仍可修改，发布后线上格式冻结）
- 实现：`crates/ferry-proto`
- 测试：`crates/ferry-proto/tests/suite/`；fuzz：`fuzz/fuzz_targets/decode.rs`

本文是协议的权威说明。实现与本文不一致时，以本文为准修正实现。

---

## 1. 连接

每对设备之间建立两条 TCP 连接，均运行在 TLS 1.3 之上，用同一套设备证书双向认证。TLS 握手使用 ALPN 标识 `screenferry`，不是本协议的连接在 TLS 阶段就会被拒绝。

| 连接 | 内容 | 单帧消息体上限 |
|---|---|---|
| 输入连接 | 键鼠事件、屏幕切换、显示器布局、心跳 | 4 KiB |
| 数据连接 | 剪贴板通告与内容、分块传输、心跳 | 64 KiB + 64 字节 |

两条连接各自独立收发，大数据传输不会阻塞键鼠事件。

## 2. 握手

```
发起方                                        接受方
  ── Hello { channel: 输入, session: 0 } ──────▶
  ◀──────────── Welcome { session: S } 或 Reject ──
  ── Hello { channel: 数据, session: S } ──────▶
  ◀──── Welcome { session: S } 或 Reject(UNKNOWN_SESSION) ──
```

- 每条连接的第一条消息必须是 `Hello`，接受方以 `Welcome` 或 `Reject` 回应。握手完成前，除这三条和 `Bye` 外的消息都是协议违规。
- 先建立输入连接，拿到会话号 `S`；数据连接带上 `S`，接受方据此把两条连接归到同一会话。
- **版本**：双方版本必须相同，否则接受方回 `Reject(INCOMPATIBLE_VERSION)`，并带上自己的版本号，便于提示用户升级哪一端。
- **能力**：`Welcome` 中的能力是双方能力的交集。只能发送已协商能力范围内的消息：
  - `PointerDelta` 需要 `RELATIVE_POINTER`
  - 剪贴板消息需要 `CLIPBOARD`
- `Hello`、`Welcome`、`Reject` 的消息体格式在所有版本中永久不变，版本不同的双方也总能完成协商。

## 3. 帧格式

```
| 类型 u16 | 长度 u32 | 消息体（长度字节） |
```

- 全部小端。帧头 6 字节，长度只计消息体。
- 帧头一到就检查：未知类型、不属于本连接的类型、超过本连接上限的长度，都立即拒绝，不等消息体。
- 消息体必须被恰好读完，有多余字节或不够都是错误。
- **任何解码错误都是协议违规**：发送 `Bye(PROTOCOL_ERROR)` 后关闭连接。

## 4. 基本编码

| 类型 | 编码 |
|---|---|
| `u8` `u16` `u32` `u64` `i32` | 小端定长 |
| `bool` | 1 字节，只能是 0 或 1 |
| `str` | `u16` 字节数 + UTF-8 内容 |
| `bytes` | `u32` 字节数 + 内容 |
| `list<T>` | `u16` 元素数 + 逐个元素 |

每个值只有一种合法编码，所以解码后再编码一定得到相同的字节。测试和 fuzz 都会检查这一点。

## 5. 消息

类型号的高字节表示类别：`0x00` 握手与心跳，`0x01` 输入，`0x02` 剪贴板，`0x03` 传输。

| 类型号 | 消息 | 连接 | 消息体 |
|---|---|---|---|
| `0x0001` | Hello | 两者 | `version u16` · `channel u8` · `capabilities u64` · `session u64` · `name str` |
| `0x0002` | Welcome | 两者 | `version u16` · `capabilities u64` · `session u64` · `name str` |
| `0x0003` | Reject | 两者 | `reason u16` · `version u16` |
| `0x0004` | Bye | 两者 | `reason u16` |
| `0x0005` | Ping | 两者 | `nonce u64` |
| `0x0006` | Pong | 两者 | `nonce u64` |
| `0x0100` | Displays | 输入 | `list<Display>`，最多 32 个 |
| `0x0101` | Enter | 输入 | `seq u32` · `position Point` · `locks u8` |
| `0x0102` | Leave | 输入 | `seq u32` |
| `0x0110` | Key | 输入 | `code u32` · `action u8` |
| `0x0120` | PointerMotion | 输入 | `position Point` |
| `0x0121` | PointerDelta | 输入 | `dx i32` · `dy i32` |
| `0x0122` | Button | 输入 | `button u8` · `pressed bool` · `position Point` |
| `0x0123` | Wheel | 输入 | `dx i32` · `dy i32` · `position Point` |
| `0x0200` | ClipboardOffer | 数据 | `seq u32` · `list<ClipboardItem>`，最多 16 个 |
| `0x0201` | ClipboardRequest | 数据 | `seq u32` · `format u8` · `transfer u32` |
| `0x0300` | Chunk | 数据 | `transfer u32` · `last bool` · `data bytes`，最多 64 KiB |
| `0x0301` | Cancel | 数据 | `transfer u32` · `reason u16` |

设备名 `name` 最多 128 字节。

### 复合类型

| 类型 | 编码 |
|---|---|
| Point | `x i32` · `y i32` |
| Rect | `x i32` · `y i32` · `width u32` · `height u32` |
| Display | `id u32` · `bounds Rect` · `scale_percent u16` · `primary bool` |
| ClipboardItem | `format u8` · `size u64` |

### 取值

| 字段 | 取值 |
|---|---|
| `channel` | 1 输入，2 数据 |
| `capabilities` | 位 0 `RELATIVE_POINTER`，位 1 `CLIPBOARD`；未知位保留，协商时自然被交集去掉 |
| `code` | USB HID 扩展用法码：高 16 位用法页，低 16 位用法 ID。键盘页 `0x07`，消费者页 `0x0C` |
| `action` | 1 按下，2 松开，3 连发 |
| `locks` | 位 0 Caps Lock，位 1 Num Lock，位 2 Scroll Lock；其他位必须为 0 |
| `button` | 1 左，2 右，3 中，4 后退，5 前进，更大的编号是额外侧键；0 非法 |
| `Wheel.dx/dy` | 一格 = 120；高精度滚动可以是它的分数 |
| `scale_percent` | 一个逻辑单位等于多少个坐标单位（百分数）：Windows 为系统缩放比例，macOS 恒为 100。双方按 `对方 / 本机` 换算移动距离 |
| `format` | 1 纯文本（UTF-8），2 HTML（UTF-8），3 RTF，4 PNG |
| Reject `reason` | 1 版本不兼容，2 会话不存在，3 正被其他设备控制 |
| Bye `reason` | 0 正常关闭，1 被新连接取代，2 协议违规，3 心跳超时 |
| Cancel `reason` | 0 主动取消，1 内容已过期，2 超过大小上限，3 源数据不可用 |

`format` 和各个 `reason` 以后可能新增取值，接收方遇到不认识的值应当按"未知"处理，不算协议违规。

## 6. 行为约定

### 心跳

收到 `Ping` 后尽快以相同 `nonce` 回 `Pong`。发送方用 `nonce` 携带发送时刻，以计算往返时间。发送间隔和超时由引擎决定：控制远端期间发送频率要高（建议 50–100 ms），既能及时发现断线，也能让对方网卡保持唤醒。

### 屏幕切换

- 光标进入对方屏幕时发 `Enter`，带递增的 `seq`、进入位置和本机锁定键状态，接收方据此同步 Caps Lock 等状态。
- 离开时发 `Leave`，带同一个 `seq`。
- 接收方收到 `Leave` **或者连接断开**时，必须释放所有仍按下的键和鼠标按键，杜绝卡键。

### 坐标

所有坐标都使用**接收方**的桌面坐标系：macOS 为点（point），Windows 为物理像素。接收方用 `Displays` 声明自己的坐标系。`Button` 和 `Wheel` 带上当时的光标位置，接收方先移动光标再执行。

### 按键

- 修饰键也是普通按键，按下与松开分别发送，不随每个按键附带修饰键掩码。
- 系统不会为注入的按键自动连发，所以连发由捕获端以 `action = 3` 显式发送。

### 剪贴板

```
A（复制的一方）                         B（粘贴的一方）
  ── ClipboardOffer { seq: 9, 文本 12B, PNG 1MB } ─▶
                    （B 上的用户按下粘贴）
  ◀──── ClipboardRequest { seq: 9, PNG, transfer: 3 } ──
  ── Chunk { transfer: 3, last: false, … } ───────▶
  ── Chunk { transfer: 3, last: true,  … } ───────▶
```

- 剪贴板一变就发 `ClipboardOffer`，只说明有哪些格式、各多大，不发内容。
- 对方真正粘贴时才发 `ClipboardRequest` 拉取。传输号由请求方分配，在本连接内唯一。
- 请求的 `seq` 已经过期时，回 `Cancel(STALE)`。
- 接收方觉得内容太大时，可以不请求，或者中途发 `Cancel(TOO_LARGE)`。

## 7. 演进规则

1. 已发布消息的消息体格式不改。需要新字段时，定义新的消息类型。
2. 新的消息类型必须由新的能力位控制。对方没有声明这个能力时，就不发这类消息。
3. 不兼容的变化（改帧格式、改已有消息）必须递增 `PROTOCOL_VERSION`。
4. 类型号一经分配不再复用；废弃的类型号保留不用。

## 8. 与 Deskflow 的主要差异

| 方面 | Deskflow | ScreenFerry |
|---|---|---|
| 消息编码 | 4 字符消息码，按格式串解析 | `u16` 类型号，严格的规范编码，帧头即可拒绝非法帧 |
| 连接 | 一条连接承载所有数据 | 输入连接与数据连接分离 |
| 按键 | X11 keysym + 修饰键掩码 + 按键号 | USB HID 物理键，修饰键独立传输 |
| 剪贴板 | 进入屏幕时推送完整内容 | 先通告，粘贴时再按格式拉取，可取消 |
| 心跳 | 固定间隔保活 | `Ping`/`Pong` 带往返时间，频率由引擎按状态调整 |
| 版本兼容 | 主次版本号 | 版本号 + 能力位协商 |
