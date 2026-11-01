//! 弹簧动画：目标一变就从当前位置和速度出发追过去，中途改目标也不会跳。

/// 一个按弹簧规律逼近目标的数值。
#[derive(Clone, Copy, Debug)]
pub(crate) struct Spring {
    pub(crate) value: f32,
    velocity: f32,
    pub(crate) target: f32,
    stiffness: f32,
    damping: f32,
}

/// 积分步长：固定的小步长，帧率高低都一样稳定。
const STEP: f32 = 1.0 / 240.0;

impl Spring {
    /// 带一点回弹，用于大小和位置。
    pub(crate) fn bouncy(value: f32) -> Self {
        Self::new(value, 320.0, 24.0)
    }

    /// 不回弹，用于透明度。
    pub(crate) fn smooth(value: f32) -> Self {
        Self::new(value, 400.0, 40.0)
    }

    fn new(value: f32, stiffness: f32, damping: f32) -> Self {
        Self {
            value,
            velocity: 0.0,
            target: value,
            stiffness,
            damping,
        }
    }

    /// 立即到位，没有动画。
    pub(crate) fn snap(&mut self, value: f32) {
        self.value = value;
        self.target = value;
        self.velocity = 0.0;
    }

    /// 前进 `dt` 秒。
    pub(crate) fn step(&mut self, dt: f32) {
        let mut left = dt.clamp(0.0, 0.1);
        while left > 0.0 {
            let h = left.min(STEP);
            let force = -self.stiffness * (self.value - self.target) - self.damping * self.velocity;
            self.velocity += force * h;
            self.value += self.velocity * h;
            left -= h;
        }
    }

    /// 是否已经停在目标上（`epsilon` 为可以忽略的距离）。
    pub(crate) fn settled(&self, epsilon: f32) -> bool {
        (self.value - self.target).abs() < epsilon && self.velocity.abs() < epsilon * 4.0
    }
}
