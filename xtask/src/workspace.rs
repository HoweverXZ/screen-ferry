//! 通过 `cargo metadata` 读取 `crates/` 下的成员及其依赖。

use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

pub(crate) struct Workspace {
    pub(crate) root: PathBuf,
    pub(crate) crates: Vec<Crate>,
}

pub(crate) struct Crate {
    pub(crate) name: String,
    pub(crate) dir: PathBuf,
    pub(crate) deps: Vec<Dep>,
}

pub(crate) struct Dep {
    pub(crate) name: String,
    pub(crate) kind: DepKind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum DepKind {
    Normal,
    Dev,
    Build,
}

pub(crate) fn load() -> Result<Workspace, String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let output = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }

    let meta: Value = serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
    let root = PathBuf::from(
        meta["workspace_root"]
            .as_str()
            .ok_or("缺少 workspace_root")?,
    );
    let crates_dir = root.join("crates");

    let mut crates = Vec::new();
    for pkg in meta["packages"].as_array().ok_or("缺少 packages")? {
        let manifest = PathBuf::from(pkg["manifest_path"].as_str().ok_or("缺少 manifest_path")?);
        let Some(dir) = manifest.parent() else {
            continue;
        };
        if dir.parent() != Some(crates_dir.as_path()) {
            continue;
        }

        let deps = pkg["dependencies"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|d| Dep {
                name: d["name"].as_str().unwrap_or_default().to_owned(),
                kind: match d["kind"].as_str() {
                    Some("dev") => DepKind::Dev,
                    Some("build") => DepKind::Build,
                    _ => DepKind::Normal,
                },
            })
            .collect();

        crates.push(Crate {
            name: pkg["name"].as_str().unwrap_or_default().to_owned(),
            dir: dir.to_path_buf(),
            deps,
        });
    }

    crates.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Workspace { root, crates })
}
