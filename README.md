# ScreenFerry

跨 Windows 与 macOS 的键鼠共享工具，同时提供剪贴板与文件同步。

目标：低内存、稳定、易扩展。

## 开发

```bash
cargo build --workspace
cargo test --workspace
cargo xtask lint
```

工程规范见 [AGENTS.md](AGENTS.md)，架构说明见 [docs/architecture.md](docs/architecture.md)。

## 试运行原型

目前是端到端验证原型：Mac 作为服务端，把键盘和鼠标（移动、按键、滚轮）共享给同一局域网内的 Windows，并双向同步剪贴板中的文本。

- 默认把 Mac 的 Command 当作 Windows 的 Ctrl，⌘C、⌘V 即复制粘贴；不需要时加 `--no-cmd-ctrl-swap`。
- 进入 Windows 时同步 Caps Lock 状态。
- 媒体键（音量加减、静音、播放/暂停、上一首、下一首）也会转发；亮度、键盘背光仍作用在 Mac 上。
- 光标在 Windows 上时，按 **Ctrl + Option + Esc** 可以强制回到 Mac。
- 剪贴板在光标切过去时才同步：在 Mac 上复制，移到 Windows 即可粘贴，反之亦然。不需要时在任意一端加 `--no-clipboard`。目前只同步纯文本。
- 测试时请先退出 Deskflow 等同类软件，两者会争抢同一套键鼠。

两台电脑之间的连接全程 TLS 1.3 加密。第一次连接时两边终端都会显示同一个 6 位配对码，核对一致后分别输入 `y` 回车确认；之后两台电脑互相记住对方，不再询问。用 `screenferry peers` 查看已配对的设备，`screenferry forget <设备名或指纹>` 删除。

1. 在 Mac 上编译两个版本（Windows 版需要 `brew install mingw-w64` 和 `rustup target add x86_64-pc-windows-gnu`）：

   ```bash
   cargo build --release -p ferry-daemon
   cargo build --release -p ferry-daemon --target x86_64-pc-windows-gnu
   ```

2. Mac 上运行服务端。首次运行需要在「系统设置 → 隐私与安全性」中给终端打开辅助功能权限（较新的 macOS 中名为“设备控制和数据访问”）：

   ```bash
   ./target/release/screenferry serve --side right
   ```

3. 把 `target/x86_64-pc-windows-gnu/release/screenferry.exe` 拷到 Windows，在终端中运行：

   ```bash
   screenferry.exe connect
   ```

   不填地址时会在局域网里自动寻找 Mac（mDNS），Mac 换了 IP 也能找回来。网络不支持组播时，改用 `connect <Mac 的局域网 IP>`。

4. 把 Mac 的光标推出屏幕右边缘，光标就会出现在 Windows 上；从 Windows 屏幕左边缘推回来即可返回。Mac 终端每 5 秒打印一次往返延迟。

断线、Windows 端退出或心跳超时时，Mac 会立即收回鼠标控制。

## 设置界面

设置界面用 Flutter 编写，工程在 `app/`，目前用模拟数据展示界面，还没有接到后台。在 Mac 上运行：

```bash
cd app && flutter run -d macos
```

需要 Flutter 3.47 和 Xcode。

## 许可证

可任选以下一种许可证使用：

- [Apache License 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)

除非你明确声明，否则你提交的任何贡献都将按上述双许可证授权，不附加其他条款或条件。
