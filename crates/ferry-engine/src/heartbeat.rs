//! 心跳：定时 Ping，按 Pong 判断连接是否存活，并统计往返延迟。

use std::fmt;
use std::time::{Duration, Instant};

/// 心跳检查的节拍。
pub(crate) const TICK: Duration = Duration::from_millis(50);
/// 光标在对方屏幕时的 Ping 间隔：要尽快发现断线，也让对方网卡保持唤醒。
const ACTIVE_INTERVAL: Duration = Duration::from_millis(100);
/// 光标在本机时的 Ping 间隔。
const IDLE_INTERVAL: Duration = Duration::from_millis(500);
/// 超过这么久没有收到 Pong，视为断线。
const TIMEOUT: Duration = Duration::from_millis(1500);
/// 延迟统计的汇报周期。
const REPORT_EVERY: Duration = Duration::from_secs(5);

pub(crate) struct Heartbeat {
    epoch: Instant,
    last_ping: Option<Instant>,
    last_pong: Instant,
    window: Window,
    last_report: Instant,
}

impl Heartbeat {
    pub(crate) fn new() -> Self {
        let now = Instant::now();
        Self {
            epoch: now,
            last_ping: None,
            last_pong: now,
            window: Window::default(),
            last_report: now,
        }
    }

    /// 到了该发 Ping 的时候，返回要携带的 nonce（发送时刻，微秒）。
    pub(crate) fn poll_ping(&mut self, active: bool) -> Option<u64> {
        let interval = if active {
            ACTIVE_INTERVAL
        } else {
            IDLE_INTERVAL
        };
        let now = Instant::now();
        if self.last_ping.is_some_and(|t| now - t < interval) {
            return None;
        }
        self.last_ping = Some(now);
        Some((now - self.epoch).as_micros() as u64)
    }

    pub(crate) fn pong(&mut self, nonce: u64) {
        let sent = self.epoch + Duration::from_micros(nonce);
        let now = Instant::now();
        if sent > now {
            return; // 不是本连接发出的 nonce
        }
        self.last_pong = now;
        self.window.record(now - sent);
    }

    pub(crate) fn timed_out(&self) -> bool {
        self.last_pong.elapsed() > TIMEOUT
    }

    /// 每隔一段时间返回一次延迟统计。
    pub(crate) fn poll_report(&mut self) -> Option<Report> {
        if self.last_report.elapsed() < REPORT_EVERY || self.window.count == 0 {
            return None;
        }
        self.last_report = Instant::now();
        Some(std::mem::take(&mut self.window).report())
    }
}

#[derive(Default)]
struct Window {
    count: u32,
    total: Duration,
    min: Option<Duration>,
    max: Duration,
}

impl Window {
    fn record(&mut self, rtt: Duration) {
        self.count += 1;
        self.total += rtt;
        self.min = Some(self.min.map_or(rtt, |m| m.min(rtt)));
        self.max = self.max.max(rtt);
    }

    fn report(&self) -> Report {
        Report {
            count: self.count,
            min: self.min.unwrap_or_default(),
            avg: self.total / self.count.max(1),
            max: self.max,
        }
    }
}

/// 一段时间内的往返延迟统计。
pub(crate) struct Report {
    count: u32,
    min: Duration,
    avg: Duration,
    max: Duration,
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ms = |d: Duration| d.as_secs_f64() * 1000.0;
        write!(
            f,
            "往返延迟（{} 次）：最小 {:.2} ms · 平均 {:.2} ms · 最大 {:.2} ms，单程约 {:.2} ms",
            self.count,
            ms(self.min),
            ms(self.avg),
            ms(self.max),
            ms(self.avg) / 2.0,
        )
    }
}
