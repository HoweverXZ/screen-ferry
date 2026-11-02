# Security / 安全问题

## English

ScreenFerry passes keystrokes, clipboard contents and files between computers, so security reports matter a lot.

**Please don't report vulnerabilities in public issues.** Use
[Report a vulnerability](https://github.com/icloudza/screen-ferry/security/advisories/new) on the Security tab instead.
Only the maintainer can see the report.

Especially relevant:

- The TLS connection, pairing and pinned device certificates
- The local connection between the settings window and the background process
- Update checks: manifest signature, rollback and expiry checks, package verification
- Anything that lets another computer on the network inject input or read data without pairing

Include the version (Settings → About), both systems, and steps to reproduce. You'll get a reply as soon as possible.

Only the latest release gets security fixes.

## 简体中文

ScreenFerry 在电脑之间传按键、剪贴板和文件，安全问题很重要。

**请不要在公开的 issue 里报告漏洞。** 到仓库 Security 页点
[Report a vulnerability](https://github.com/icloudza/screen-ferry/security/advisories/new) 私下报告，只有维护者能看到。

特别关心这些方面：

- TLS 连接、配对和固定的设备证书
- 设置界面和后台之间的本机通信
- 版本更新：清单签名、防回滚和过期检查、安装包校验
- 没有配对的电脑就能注入键鼠或读到数据的任何情况

请写上版本号（「设置 → 关于」里能看到）、两台电脑的系统和复现步骤。会尽快回复。

只有最新的正式版会修安全问题。
