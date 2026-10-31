//! 依赖规则：AGENTS.md 第 4、5 节。

use crate::workspace::{DepKind, Workspace};

/// crate 所在层级。新增 crate 必须先在这里登记。
pub(crate) fn layer(name: &str) -> Option<u8> {
    match name {
        "ferry-types" => Some(0),
        "ferry-proto" | "ferry-core" | "ferry-trust" | "ferry-platform" => Some(1),
        "ferry-net" | "ferry-store" | "ferry-ipc" => Some(2),
        n if is_platform_impl(n) => Some(2),
        "ferry-engine" => Some(3),
        "ferry-daemon" | "ferry-settings" | "ferry-service" => Some(4),
        _ => None,
    }
}

/// `ferry-platform-*`：具体平台实现，允许 `unsafe` 和 `cfg(target_os)`。
pub(crate) fn is_platform_impl(name: &str) -> bool {
    name.starts_with("ferry-platform-")
}

/// 允许出现 `cfg(target_os)` 的 crate：平台实现和应用。
pub(crate) fn may_use_target_os(name: &str) -> bool {
    is_platform_impl(name) || layer(name) == Some(4)
}

const SETTINGS_ALLOWED: &[&str] = &["ferry-ipc", "ferry-types"];

pub(crate) fn check(ws: &Workspace, out: &mut Vec<String>) {
    for c in &ws.crates {
        let Some(own) = layer(&c.name) else {
            out.push(format!(
                "{}：未登记层级，先在 xtask/src/deps.rs 的 layer() 中登记",
                c.name
            ));
            continue;
        };

        for d in c.deps.iter().filter(|d| d.name.starts_with("ferry-")) {
            let Some(dep) = layer(&d.name) else { continue };

            if dep >= own {
                out.push(format!(
                    "{}（L{own}）不能依赖 {}（L{dep}）：只能依赖更低层",
                    c.name, d.name
                ));
            }
            if c.name == "ferry-engine" && d.kind != DepKind::Dev && is_platform_impl(&d.name) {
                out.push(format!(
                    "ferry-engine 不能依赖具体平台实现 {}：只依赖 ferry-platform 的 trait",
                    d.name
                ));
            }
            if c.name == "ferry-settings" && !SETTINGS_ALLOWED.contains(&d.name.as_str()) {
                out.push(format!(
                    "ferry-settings 不能依赖 {}：只能通过 ferry-ipc 与后台通信",
                    d.name
                ));
            }
        }
    }
}
