//! 日志。
//!
//! 常驻运行时总是写运行日志文件（`ferry-store` 指定的位置），从终端运行时同时写到终端
//! （Windows 上常驻运行不接终端）；其他命令只写到终端。
//!
//! - 时间用本机时间（`2026-11-02 06:12:03.104`），提问题时对得上“几点出的问题”。
//!   时区在启动时取一次：之后进程里有了多个线程，有的系统就不让再取了。
//! - 文件写满 2 MB 时改名成上一卷（`screenferry.log.1`）再开一个，只留这一卷。
//! - “详细记录”（DEBUG）可以在运行中打开、关闭，见 [`set_verbose`]。

use std::fs::{self, File, OpenOptions};
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use ferry_store::Store;
use time::UtcOffset;
use time::macros::format_description;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::fmt::time::OffsetTime;
use tracing_subscriber::fmt::writer::{BoxMakeWriter, MakeWriterExt};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{Registry, reload};

use crate::cli::{Mode, Options};

/// 运行日志一卷的大小上限。
const LOG_LIMIT: u64 = 2 * 1024 * 1024;

/// 运行中改日志级别用。
static LEVEL: OnceLock<reload::Handle<LevelFilter, Registry>> = OnceLock::new();
/// 启动时取到的本机时区。
static OFFSET: OnceLock<UtcOffset> = OnceLock::new();

pub(crate) fn init(options: &Options) {
    let offset =
        *OFFSET.get_or_init(|| UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC));
    let timer = OffsetTime::new(
        offset,
        format_description!("[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3]"),
    );
    let (level, handle) = reload::Layer::new(if options.verbose {
        LevelFilter::DEBUG
    } else {
        LevelFilter::INFO
    });
    let _ = LEVEL.set(handle);

    let terminal = io::stderr().is_terminal();
    let file = matches!(options.mode, Mode::Resident { .. })
        .then(|| log_file(options))
        .flatten()
        .map(Mutex::new);
    // Windows 的控制台窗口不一定认颜色控制码，不加颜色；写进文件时也不加。
    let ansi = terminal && file.is_none() && !cfg!(target_os = "windows");
    let writer = match file {
        Some(file) if terminal => BoxMakeWriter::new(file.and(io::stderr)),
        Some(file) => BoxMakeWriter::new(file),
        None => BoxMakeWriter::new(io::stderr),
    };
    let output = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_timer(timer)
        .with_ansi(ansi)
        .with_writer(writer);
    tracing_subscriber::registry()
        .with(level)
        .with(output)
        .init();
}

/// 打开或关闭详细记录。
pub(crate) fn set_verbose(on: bool) {
    if let Some(handle) = LEVEL.get() {
        let level = if on {
            LevelFilter::DEBUG
        } else {
            LevelFilter::INFO
        };
        let _ = handle.reload(level);
    }
}

/// 本机时区与 UTC 相差的秒数。
pub(crate) fn utc_offset() -> i32 {
    OFFSET.get().map_or(0, |o| o.whole_seconds())
}

fn log_file(options: &Options) -> Option<LogFile> {
    let store = match &options.data_dir {
        Some(dir) => Store::open(dir),
        None => Store::open_default(),
    }
    .ok()?;
    fs::create_dir_all(store.logs_dir()).ok()?;
    LogFile::open(store.log_file(), store.previous_log_file()).ok()
}

/// 运行日志文件：写满 [`LOG_LIMIT`] 时改名成上一卷，再开一个新的。
struct LogFile {
    path: PathBuf,
    previous: PathBuf,
    file: Option<File>,
    len: u64,
}

impl LogFile {
    fn open(path: PathBuf, previous: PathBuf) -> io::Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let len = file.metadata()?.len();
        let mut log = Self {
            path,
            previous,
            file: Some(file),
            len,
        };
        if log.len > LOG_LIMIT {
            log.rotate();
        }
        Ok(log)
    }

    fn rotate(&mut self) {
        // 先关掉再改名：Windows 上不能给打开着的文件改名。
        self.file = None;
        let _ = fs::rename(&self.path, &self.previous);
        self.file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.path)
            .ok();
        self.len = 0;
    }
}

impl Write for LogFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // 每条日志一次写进来，在两条之间换卷。
        if self.len > 0 && self.len + buf.len() as u64 > LOG_LIMIT {
            self.rotate();
        }
        // 文件打不开时丢掉这条日志，不影响程序运行。
        let Some(file) = self.file.as_mut() else {
            return Ok(buf.len());
        };
        let written = file.write(buf)?;
        self.len += written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.as_mut().map_or(Ok(()), File::flush)
    }
}
