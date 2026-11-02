# Contributing / 参与贡献

[English](#english) | [简体中文](#简体中文)

## English

Thanks for helping out. Bug reports, ideas and pull requests are all welcome.

### Reporting a problem

Open a [bug report](https://github.com/icloudza/screen-ferry/issues/new?template=bug_report.yml) and attach a diagnostics file
(Log → Diagnostics → Export Diagnostics). It has device names, user names and file names replaced with placeholders,
and contains no clipboard contents or keys. Security issues go through [SECURITY.md](SECURITY.md), not public issues.

### Before writing code

- For anything bigger than a small fix, open an issue first so we can agree on the approach.
- Read [AGENTS.md](../AGENTS.md). It is the engineering spec: directory layout, crate layers, naming, where tests go.
  `cargo xtask lint` checks most of it. Sections 3–10 are frozen; changing them needs an ADR in `docs/adr/` first.
- How to build: see "Build from source" in the [README](../README.md#build-from-source).

### Pull requests

- One topic per pull request, with tests for the change.
- Run the checks in AGENTS.md section 12 before pushing. CI runs the same on macOS, Windows and the Flutter app.
- Put user-visible changes in `CHANGELOG.md` under `[Unreleased]`.
- Commit messages: `type: description`, where type is `feat`, `fix`, `docs`, `refactor`, `test` or `chore`.
  The description may be in Chinese or English.
- UI text needs both languages (`app/lib/l10n/app_zh.arb` and `app_en.arb`); README changes go in both READMEs.

### License

ScreenFerry is dual-licensed under MIT OR Apache-2.0. Unless you say otherwise, anything you contribute is licensed the same way.

## 简体中文

谢谢你来帮忙。问题反馈、功能建议、PR 都欢迎。

### 反馈问题

开一个[问题反馈](https://github.com/icloudza/screen-ferry/issues/new?template=bug_report.yml)，附上诊断文件
（「日志 → 运行日志」里点“导出诊断文件”）。诊断文件里的设备名、用户名和文件名已经换成代号，不含剪贴板内容和密钥。
安全问题请按 [SECURITY.md](SECURITY.md) 私下报告，不要发公开的 issue。

### 写代码之前

- 比小修复大的改动，先开 issue 说一下想法，商量好做法再动手。
- 先读 [AGENTS.md](../AGENTS.md)：目录、crate 分层、命名、测试放哪儿都在里面，大部分由 `cargo xtask lint` 自动检查。
  第 3–10 节已冻结，要改得先在 `docs/adr/` 写决策记录。
- 怎么构建：见 [README](../README.zh-CN.md#从源码构建) 的「从源码构建」。

### 提 PR

- 一个 PR 只做一件事，带上对应的测试。
- 推送前跑一遍 AGENTS.md 第 12 节的检查。CI 会在 macOS、Windows 和设置界面上跑同样的检查。
- 用户看得到的改动记进 `CHANGELOG.md` 的 `[Unreleased]`。
- 提交信息写成 `类型: 说明`，类型用 `feat`、`fix`、`docs`、`refactor`、`test`、`chore`；说明中英文都可以。
- 界面文字中英文都要有（`app/lib/l10n/app_zh.arb` 和 `app_en.arb`）；改 README 要两份一起改。

### 许可证

ScreenFerry 采用 MIT OR Apache-2.0 双许可证。除非你另外声明，你贡献的内容也按同样的方式授权。
