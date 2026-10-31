//! 文件读写小工具：错误带上路径，写入先写临时文件再改名，中途断电也不会留下半个文件。

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use crate::error::{Error, Result};

/// 读取文件；不存在时返回 `None`。
pub(crate) fn read(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(data) => Ok(Some(data)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io(path, e)),
    }
}

/// 原子地替换文件内容，必要时创建上级目录。
pub(crate) fn write(path: &Path, data: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| io(dir, e))?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = Path::new(&tmp);
    fs::write(tmp, data).map_err(|e| io(tmp, e))?;
    fs::rename(tmp, path).map_err(|e| io(path, e))
}

pub(crate) fn io(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.to_owned(),
        source,
    }
}
