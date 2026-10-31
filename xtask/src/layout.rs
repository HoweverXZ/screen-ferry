//! 结构规则：AGENTS.md 第 5–9 节中可以静态检查的部分。

use std::fs;
use std::path::{Path, PathBuf};

use toml::{Table, Value};

use crate::deps::{is_platform_impl, may_use_target_os};
use crate::workspace::{Crate, Workspace};

const DEP_TABLES: &[&str] = &["dependencies", "dev-dependencies", "build-dependencies"];
const INHERITED_FIELDS: &[&str] = &["version", "edition", "rust-version", "license", "publish"];

pub(crate) fn check(ws: &Workspace, out: &mut Vec<String>) {
    for c in &ws.crates {
        let ctx = Ctx {
            ws,
            c,
            out: &mut *out,
        };
        ctx.check_all();
    }
}

struct Ctx<'a> {
    ws: &'a Workspace,
    c: &'a Crate,
    out: &'a mut Vec<String>,
}

impl Ctx<'_> {
    fn check_all(mut self) {
        self.naming();
        self.sources();
        self.unsafe_forbidden();
        self.tests();
        self.manifest();
    }

    fn report(&mut self, path: &Path, msg: impl AsRef<str>) {
        let rel = path.strip_prefix(&self.ws.root).unwrap_or(path);
        self.out
            .push(format!("{}：{}", rel.display(), msg.as_ref()));
    }

    fn naming(&mut self) {
        let c = self.c;
        let dir_name = c
            .dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if dir_name != c.name {
            self.report(&c.dir, format!("目录名必须与 package 名 `{}` 一致", c.name));
        }
        if !c.name.starts_with("ferry-") {
            self.report(&c.dir, "crate 名必须以 `ferry-` 开头");
        }
    }

    fn sources(&mut self) {
        for file in rs_files(&self.c.dir.join("src")) {
            let stem = file
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            if stem == "mod" {
                self.report(&file, "不使用 mod.rs，改用 foo.rs + foo/");
            }
            if stem.ends_with("_test")
                || stem.ends_with("_tests")
                || stem == "test"
                || stem == "tests"
            {
                self.report(&file, "src/ 中不能放测试文件，移到 tests/suite/");
            }

            let text = fs::read_to_string(&file).unwrap_or_default();
            if text.contains("cfg(test)") {
                self.report(&file, "src/ 中不能写 #[cfg(test)]，测试移到 tests/suite/");
            }
            if text.contains("target_os") && !may_use_target_os(&self.c.name) {
                self.report(
                    &file,
                    "只有 ferry-platform-* 和应用 crate 可以使用 cfg(target_os)",
                );
            }
        }
    }

    fn unsafe_forbidden(&mut self) {
        if is_platform_impl(&self.c.name) {
            return;
        }
        for root in ["src/lib.rs", "src/main.rs"] {
            let path = self.c.dir.join(root);
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            if !text.contains("#![forbid(unsafe_code)]") {
                self.report(&path, "必须声明 #![forbid(unsafe_code)]");
            }
        }
    }

    fn tests(&mut self) {
        let tests = self.c.dir.join("tests");
        let Ok(entries) = fs::read_dir(&tests) else {
            return;
        };

        for entry in entries.flatten() {
            if entry.file_name() != "suite" {
                self.report(&entry.path(), "tests/ 下只允许 suite/ 目录");
            }
        }

        let suite = tests.join("suite");
        if suite.is_dir() && !suite.join("main.rs").is_file() {
            self.report(&suite, "缺少 main.rs");
        }
        for file in rs_files(&suite) {
            if file.file_name().is_some_and(|n| n == "mod.rs") {
                self.report(&file, "不使用 mod.rs，改用 foo.rs + foo/");
            }
        }
    }

    fn manifest(&mut self) {
        let path = self.c.dir.join("Cargo.toml");
        let doc: Table = match fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|text| text.parse().map_err(|e: toml::de::Error| e.to_string()))
        {
            Ok(doc) => doc,
            Err(e) => return self.report(&path, format!("无法解析：{e}")),
        };

        if !inherits(doc.get("lints")) {
            self.report(&path, "必须写 [lints] workspace = true");
        }

        if self.c.dir.join("src/lib.rs").is_file() && !test_disabled(doc.get("lib")) {
            self.report(&path, "必须写 [lib] test = false：src/ 中不放测试");
        }
        let bins = doc.get("bin").and_then(Value::as_array);
        if self.c.dir.join("src/main.rs").is_file() && bins.is_none() {
            self.report(
                &path,
                "可执行文件必须用 [[bin]] 显式声明名称，并写 test = false",
            );
        }
        for bin in bins.into_iter().flatten() {
            if !test_disabled(Some(bin)) {
                self.report(&path, "每个 [[bin]] 必须写 test = false：src/ 中不放测试");
            }
        }

        let package = doc.get("package");
        for field in INHERITED_FIELDS {
            if !inherits(package.and_then(|p| p.get(field))) {
                self.report(&path, format!("package.{field} 必须继承 workspace"));
            }
        }

        let mut sections: Vec<(String, &Value)> = DEP_TABLES
            .iter()
            .filter_map(|k| doc.get(*k).map(|t| ((*k).to_owned(), t)))
            .collect();

        if let Some(targets) = doc.get("target").and_then(Value::as_table) {
            if !may_use_target_os(&self.c.name) {
                self.report(
                    &path,
                    "只有 ferry-platform-* 和应用 crate 可以声明平台相关依赖",
                );
            }
            for (cfg, table) in targets {
                for k in DEP_TABLES {
                    if let Some(t) = table.get(*k) {
                        sections.push((format!("target.{cfg}.{k}"), t));
                    }
                }
            }
        }

        for (section, table) in sections {
            for (name, spec) in table.as_table().into_iter().flatten() {
                if !inherits(Some(spec)) {
                    self.report(
                        &path,
                        format!("[{section}] {name} 必须写成 `{name}.workspace = true`"),
                    );
                }
            }
        }
    }
}

/// 目标是否写了 `test = false`。
fn test_disabled(target: Option<&Value>) -> bool {
    target.and_then(|t| t.get("test")).and_then(Value::as_bool) == Some(false)
}

/// 值是否为 `{ workspace = true, … }`。
fn inherits(value: Option<&Value>) -> bool {
    value
        .and_then(|v| v.get("workspace"))
        .and_then(Value::as_bool)
        == Some(true)
}

fn rs_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for path in entries.flatten().map(|e| e.path()) {
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}
