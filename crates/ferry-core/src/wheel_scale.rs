//! 滚轮换算：服务端把滚动发给一台设备之前，按那台设备的设置反向、乘倍数（ADR 0009）。

use ferry_types::DeviceSettings;

/// 一台设备的滚轮换算。乘倍数后不足 1 的部分累积到下一次同方向的滚动。
#[derive(Clone, Debug)]
pub struct WheelScale {
    flip_x: bool,
    flip_y: bool,
    percent: i64,
    /// 换算后不足 1 的余数（乘以 100），方向与本机的滚动相同。
    carry: (i64, i64),
}

impl WheelScale {
    pub fn new(settings: &DeviceSettings) -> Self {
        Self {
            flip_x: settings.reverse_horizontal,
            flip_y: settings.reverse_vertical,
            percent: i64::from(settings.scroll_speed.percent()),
            carry: (0, 0),
        }
    }

    /// 一次滚动（`dy` 为正向上，`dx` 为正向右，单位同协议），返回要发给对方的值。
    /// 两个都是 0 时调用方不必发。
    pub fn apply(&mut self, dx: i32, dy: i32) -> (i32, i32) {
        let x = step(&mut self.carry.0, dx, self.percent);
        let y = step(&mut self.carry.1, dy, self.percent);
        (
            if self.flip_x { -x } else { x },
            if self.flip_y { -y } else { y },
        )
    }
}

/// 一个方向上的换算。换了方向时丢掉之前的余数。
fn step(carry: &mut i64, v: i32, percent: i64) -> i32 {
    if v == 0 {
        return 0;
    }
    if *carry != 0 && (*carry > 0) != (v > 0) {
        *carry = 0;
    }
    let total = i64::from(v) * percent + *carry;
    *carry = total % 100;
    (total / 100).clamp(i64::from(i32::MIN + 1), i64::from(i32::MAX)) as i32
}
