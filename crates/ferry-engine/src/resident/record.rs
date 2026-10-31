//! 给设置界面看的记录：日志、每小时的活动、往返延迟。都只保存在内存里，后台重启即清空。

use std::collections::{BTreeMap, VecDeque};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ferry_ipc::{HourActivity, LogEntry, LogIcon, LogKind};

/// 日志最多保留的条数。
const LOG_CAPACITY: usize = 200;
/// 活动统计保留的小时数。
const ACTIVITY_HOURS: i64 = 48;
/// 往返延迟保留的秒数。
const RTT_SECONDS: usize = 32;

/// 当前时刻（Unix 毫秒）。
pub(crate) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// 最近的日志，最新的在前。
#[derive(Default)]
pub(crate) struct Log {
    entries: VecDeque<LogEntry>,
    next_id: u64,
}

impl Log {
    pub(crate) fn push(
        &mut self,
        kind: LogKind,
        icon: LogIcon,
        text: impl Into<String>,
        detail: impl Into<String>,
    ) -> LogEntry {
        let entry = LogEntry {
            id: self.next_id,
            kind,
            icon,
            text: text.into(),
            detail: detail.into(),
            at: now_ms(),
        };
        self.next_id += 1;
        self.entries.push_front(entry.clone());
        self.entries.truncate(LOG_CAPACITY);
        entry
    }

    pub(crate) fn entries(&self) -> Vec<LogEntry> {
        self.entries.iter().cloned().collect()
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }
}

/// 按整点小时累计的活动。
#[derive(Default)]
pub(crate) struct Activity {
    hours: BTreeMap<i64, HourActivity>,
}

impl Activity {
    pub(crate) fn switched(&mut self) {
        self.current().switches += 1;
    }

    pub(crate) fn clipped(&mut self) {
        self.current().clips += 1;
    }

    pub(crate) fn connected_for(&mut self, secs: u32) {
        let bucket = self.current();
        bucket.connected_secs = bucket.connected_secs.saturating_add(secs).min(3600);
    }

    /// 按时间先后列出最近 48 小时中有活动的小时。
    pub(crate) fn recent(&self) -> Vec<HourActivity> {
        self.hours.values().copied().collect()
    }

    fn current(&mut self) -> &mut HourActivity {
        let hour = now_ms() / 1000 / 3600 * 3600;
        self.hours.retain(|&h, _| h > hour - ACTIVITY_HOURS * 3600);
        self.hours.entry(hour).or_insert(HourActivity {
            hour,
            switches: 0,
            clips: 0,
            connected_secs: 0,
        })
    }
}

/// 往返延迟：每秒取一次平均，保留最近 32 秒。
#[derive(Default)]
pub(crate) struct Rtt {
    history: VecDeque<f64>,
    total: Duration,
    count: u32,
}

impl Rtt {
    pub(crate) fn record(&mut self, rtt: Duration) {
        self.total += rtt;
        self.count += 1;
    }

    /// 结束这一秒。这一秒有测量时返回 `true`。
    pub(crate) fn roll(&mut self) -> bool {
        if self.count == 0 {
            return false;
        }
        let avg = self.total.as_secs_f64() * 1000.0 / f64::from(self.count);
        self.history.push_back((avg * 100.0).round() / 100.0);
        while self.history.len() > RTT_SECONDS {
            self.history.pop_front();
        }
        self.total = Duration::ZERO;
        self.count = 0;
        true
    }

    pub(crate) fn history(&self) -> Vec<f64> {
        self.history.iter().copied().collect()
    }

    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }
}
