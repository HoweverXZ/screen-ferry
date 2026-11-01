//! 程序挪动光标之后，系统报告的下一次鼠标移动里会混进这段挪动的距离（macOS 就是这样）。
//!
//! 光标切到对方时，本机光标被挪到屏幕中央藏起来；回来时再挪到边上。不处理的话，
//! 挪动之后的第一个移动事件会带着几百上千个点的位移：切过去的那一刻对方光标被一下推出屏幕，
//! 又切了回来，回到本机时也被推到屏幕中间。
//!
//! 所以记下挪了多远，在接下来很短的时间里，看到某次位移里混着这段距离就把它减掉，
//! 只留下手上真实的那一点移动。

use ferry_types::Point;

/// 挪动之后多久之内还会出现混着挪动距离的位移（毫秒）。
const WINDOW_MS: u64 = 250;
/// 挪动的距离小于这么多个点时不处理：混进去也看不出来。
const MIN_JUMP: i64 = 8;
/// 减掉挪动距离之后，剩下的位移不超过这么多才算“混着挪动距离”（至少这么多）。
const MIN_SLACK: i64 = 64;

/// 跟踪最近一次挪动光标，把它从随后的位移里减掉。
#[derive(Clone, Debug, Default)]
pub struct WarpJump {
    /// 挪动的距离，以及到什么时候为止还要留意。
    pending: Option<(i64, i64, u64)>,
}

impl WarpJump {
    pub fn new() -> Self {
        Self::default()
    }

    /// 程序把光标从 `from` 挪到了 `to`。`now_ms` 是此刻的毫秒数，起点任意，只要前后一致。
    pub fn warped(&mut self, from: Point, to: Point, now_ms: u64) {
        let (jx, jy) = (
            i64::from(to.x) - i64::from(from.x),
            i64::from(to.y) - i64::from(from.y),
        );
        self.pending = (jx.abs() + jy.abs() >= MIN_JUMP).then_some((jx, jy, now_ms + WINDOW_MS));
    }

    /// 一次移动的位移。混着挪动距离时减掉它，返回真正的位移。
    ///
    /// 剩下的部分要明显比挪动距离小（不超过它的三分之一，至少 64 个点），才认为是混着挪动距离；
    /// 真实的一次移动很少有几百个点，不会被误减。减过一次就不再留意。
    pub fn correct(&mut self, dx: i32, dy: i32, now_ms: u64) -> (i32, i32) {
        let Some((jx, jy, until)) = self.pending else {
            return (dx, dy);
        };
        if now_ms > until {
            self.pending = None;
            return (dx, dy);
        }
        let (rx, ry) = (i64::from(dx) - jx, i64::from(dy) - jy);
        let slack = ((jx.abs() + jy.abs()) / 3).max(MIN_SLACK);
        if rx.abs() + ry.abs() > slack {
            // 不像混着挪动距离：原样交出去，继续留意下一次。
            return (dx, dy);
        }
        self.pending = None;
        (
            rx.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
            ry.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        )
    }

    /// 是否还在留意一次挪动。
    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }
}
