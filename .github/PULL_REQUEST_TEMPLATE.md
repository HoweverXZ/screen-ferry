<!-- 中文、英文都可以 / Chinese or English is fine -->

## 改了什么 / What changed



## 为什么 / Why

<!-- 关联的 issue 写在这里，例如 Closes #编号 / Link the issue, e.g. Closes #number -->

## 怎么验证的 / How it was tested

<!-- 跨两台电脑的改动，写清两台各是什么系统、哪台是键鼠所在的那台
     For changes between two computers, say which systems and which one has the keyboard and mouse -->

- [ ] macOS
- [ ] Windows

## 检查清单 / Checklist

- [ ] 本地跑过 [AGENTS.md](https://github.com/icloudza/screen-ferry/blob/main/AGENTS.md) 第 12 节的检查（改了 `app/` 的还有 Flutter 那三项）/ Ran the checks in AGENTS.md section 12
- [ ] 用户看得到的改动记进了 `CHANGELOG.md` 的 `[Unreleased]` / User-visible changes are in `CHANGELOG.md` under `[Unreleased]`
- [ ] 改了界面文字的，中英文 ARB 都改了；改了 README 的，两份都改了 / UI strings updated in both ARB files; both READMEs updated
- [ ] 改了线协议、本机通信或配置结构的，更新了 `docs/protocol.md` / `docs/ipc.md`，配置升了 `schema_version` 并带迁移 / Protocol, IPC or config changes are documented, with a `schema_version` bump and migration
- [ ] 动到 AGENTS.md 第 3–10 节结构规则的，先写了 ADR / Changes to the structure rules in sections 3–10 come with an ADR
- [ ] 界面改动附了浅色、深色截图 / UI changes include light and dark screenshots
