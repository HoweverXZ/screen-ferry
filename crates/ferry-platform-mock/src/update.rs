//! 自己更新：`Updater` 的测试实现，记下调用，不真的安装。

use std::path::Path;
use std::sync::{Arc, Mutex};

use ferry_platform::{Error, Prepared, Result, Updater};

#[derive(Debug, Default)]
struct Inner {
    blocked: Option<&'static str>,
    cancel: bool,
    calls: Vec<String>,
}

/// 克隆后共用同一份记录。缺省能安装，更新包叫 `test`，像 macOS 一样由自己重启。
#[derive(Clone, Debug, Default)]
pub struct MockUpdater(Arc<Mutex<Inner>>);

impl MockUpdater {
    pub fn new() -> Self {
        Self::default()
    }

    /// 之后 `can_install` 返回这个原因。
    pub fn block(&self, reason: &'static str) {
        self.0.lock().unwrap().blocked = Some(reason);
    }

    /// 之后 `launch` 像用户在 UAC 里点了“否”。
    pub fn cancel_launch(&self) {
        self.0.lock().unwrap().cancel = true;
    }

    /// 到目前为止的调用，例如 `prepare 0.2.0`、`launch`。
    pub fn calls(&self) -> Vec<String> {
        self.0.lock().unwrap().calls.clone()
    }

    fn record(&self, call: String) {
        self.0.lock().unwrap().calls.push(call);
    }
}

impl Updater for MockUpdater {
    fn platform(&self) -> Option<&'static str> {
        Some("test")
    }

    fn can_install(&self) -> Result<()> {
        match self.0.lock().unwrap().blocked {
            Some(reason) => Err(Error::Unsupported(reason)),
            None => Ok(()),
        }
    }

    fn proxy(&self) -> Option<String> {
        None
    }

    fn prepare(&self, package: &Path, version: &str) -> Result<Prepared> {
        self.record(format!("prepare {version}"));
        Ok(Prepared {
            target: package.to_owned(),
            backup: None,
            restart_self: true,
        })
    }

    fn launch(&self, _prepared: &Prepared) -> Result<()> {
        self.record("launch".into());
        if self.0.lock().unwrap().cancel {
            return Err(Error::Cancelled);
        }
        Ok(())
    }

    fn confirm(&self, _prepared: &Prepared) {
        self.record("confirm".into());
    }

    fn rollback(&self, _prepared: &Prepared) -> Result<()> {
        self.record("rollback".into());
        Ok(())
    }
}
