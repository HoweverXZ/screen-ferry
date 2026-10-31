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

目前是端到端验证原型：Mac 作为服务端，把鼠标（移动、按键、滚轮）共享给同一局域网内的 Windows。键盘还未转发。

1. 在 Mac 上编译两个版本（Windows 版需要 `brew install mingw-w64` 和 `rustup target add x86_64-pc-windows-gnu`）：

   ```bash
   cargo build --release -p ferry-daemon
   cargo build --release -p ferry-daemon --target x86_64-pc-windows-gnu
   ```

2. Mac 上运行服务端。首次运行需要在「系统设置 → 隐私与安全性 → 辅助功能」中允许终端：

   ```bash
   ./target/release/screenferry serve --side right
   ```

3. 把 `target/x86_64-pc-windows-gnu/release/screenferry.exe` 拷到 Windows，在终端中运行：

   ```bash
   screenferry.exe connect <Mac 的局域网 IP>
   ```

4. 把 Mac 的光标推出屏幕右边缘，光标就会出现在 Windows 上；从 Windows 屏幕左边缘推回来即可返回。Mac 终端每 5 秒打印一次往返延迟。

断线、Windows 端退出或心跳超时时，Mac 会立即收回鼠标控制。

## 许可证

可任选以下一种许可证使用：

- [Apache License 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)

除非你明确声明，否则你提交的任何贡献都将按上述双许可证授权，不附加其他条款或条件。
