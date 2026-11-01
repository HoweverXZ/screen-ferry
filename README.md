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
- 两台电脑可以互传文件和文件夹：`screenferry send <文件或文件夹>...` 请正在运行的 ScreenFerry 发给它连着的电脑，对方收到后放在“下载”文件夹，重名时自动编号。断线后重新连上会接着传。拖到屏幕边缘发送的方式正在做（见 ADR 0005）。
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

## 常驻运行与设置界面

`screenferry` 不带参数时常驻运行：按数据目录中的 `config.toml` 工作（macOS 默认当服务端），
菜单栏有图标，可以暂停或继续共享、打开设置、退出；配对、方向、开关都在设置界面里完成。
已经有一份在运行时，再运行一次会请它打开设置界面。

在 Mac 上打包成一个应用（后台 + 设置界面）：

```bash
cargo xtask bundle
open target/bundle/macos/ScreenFerry.app
```

产物在 `target/bundle/macos/ScreenFerry.app`，可以拖进“应用程序”文件夹。第一次运行时系统会请你
在「系统设置 → 隐私与安全性 → 辅助功能」中允许 ScreenFerry 控制键盘和鼠标。

签名：钥匙串里有“Developer ID Application”证书时自动用它签名（也可以用环境变量
`SCREENFERRY_SIGN_IDENTITY` 指定），系统按“团队 + bundle id”认这个应用，重新打包后已授予的权限不会丢；
没有证书时用临时签名，每次重新打包都要重新授权。要清掉 ScreenFerry 的辅助功能授权重新来过：

```bash
tccutil reset Accessibility io.github.icloudza.screenferry
```

设置里的“开机自动启动”会在 `~/Library/LaunchAgents/`
登记，登录时安静地启动，不弹出设置界面。没有终端时日志写在 `~/Library/Caches/ScreenFerry/logs/`。

设置界面用 Flutter 编写，工程在 `app/`，需要 Flutter 3.47 和 Xcode。它读取后台写在
`~/Library/Caches/ScreenFerry/ipc.json` 中的端口和令牌连接后台（协议见 [docs/ipc.md](docs/ipc.md)）；
后台没在运行时显示“后台没有运行”，后台启动后自动连上。另一台电脑第一次连接时，
配对码在设置界面里核对；没有打开设置界面时，后台会自动打开它。

## Windows

不想在 Windows 上装开发工具时，可以直接下载 CI 打好的包：GitHub 的 Actions 页面里打开最近一次运行，
在 Artifacts 中下载 `ScreenFerry-windows-<提交号>`（保留 14 天），解压后双击 `screenferry.exe`。

自己编译时，Windows 版的设置界面必须在 Windows 上编译（Flutter 不能在 macOS 上交叉编译 Windows 应用）。需要：

- Flutter 3.47（`flutter doctor` 全部打勾）；
- Visual Studio 2022，安装“使用 C++ 的桌面开发”；
- Rust（rustup，默认的 MSVC 工具链）。

在仓库根目录运行：

```bash
cargo xtask bundle
```

产物是 `target/bundle/windows/ScreenFerry/` 目录：`screenferry.exe` 是后台，`settings\` 里是设置界面。
双击 `screenferry.exe` 即可：任务栏右下角出现图标，并打开设置界面。设置里的“开机自动启动”
写在当前用户的启动项里（`HKCU\...\Run`）。日志在 `%LOCALAPPDATA%\ScreenFerry\logs\`。

## 许可证

可任选以下一种许可证使用：

- [Apache License 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)

除非你明确声明，否则你提交的任何贡献都将按上述双许可证授权，不附加其他条款或条件。
