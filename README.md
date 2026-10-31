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

## 许可证

可任选以下一种许可证使用：

- [Apache License 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)

除非你明确声明，否则你提交的任何贡献都将按上述双许可证授权，不附加其他条款或条件。
