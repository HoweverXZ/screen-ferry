use ferry_core::WarpJump;
use ferry_types::Point;

/// 实测的那台 Mac：主显示器 2880 × 1620 点，光标藏起来时停在中央。
const PARK: Point = Point::new(1440, 810);

fn p(x: i32, y: i32) -> Point {
    Point::new(x, y)
}

#[test]
fn leaving_through_each_edge_keeps_only_the_real_movement() {
    // 从四条边离开：挪到中央之后的第一个位移混着挪动距离，再加上手上真实的那一点。
    let cases = [
        ("左", p(0, 500), (-7, 1)),
        ("右", p(2879, 300), (9, -2)),
        ("上", p(1200, 0), (3, -6)),
        ("下", p(700, 1619), (-1, 8)),
    ];
    for (side, from, real) in cases {
        let mut warp = WarpJump::new();
        warp.warped(from, PARK, 1000);
        let mixed = (real.0 + (PARK.x - from.x), real.1 + (PARK.y - from.y));
        assert_eq!(warp.correct(mixed.0, mixed.1, 1004), real, "从{side}边离开");
        assert!(!warp.is_pending());
        // 之后的位移原样交出去。
        assert_eq!(warp.correct(-5, 2, 1010), (-5, 2), "从{side}边离开之后");
    }
}

#[test]
fn coming_back_to_each_edge_keeps_only_the_real_movement() {
    // 回来时从中央挪到边上，第一个位移同样混着这段距离。
    for to in [p(1, 600), p(2878, 200), p(1300, 1), p(900, 1618)] {
        let mut warp = WarpJump::new();
        warp.warped(PARK, to, 0);
        let (jx, jy) = (to.x - PARK.x, to.y - PARK.y);
        assert_eq!(warp.correct(jx + 4, jy - 3, 3), (4, -3), "回到 {to:?}");
    }
}

#[test]
fn a_fast_flick_mixed_with_the_jump_is_still_corrected() {
    let mut warp = WarpJump::new();
    warp.warped(p(0, 500), PARK, 0);
    // 甩得很快：一次 150 个点。
    assert_eq!(warp.correct(-150 + 1440, 20 + 310, 2), (-150, 20));
}

#[test]
fn ordinary_movement_before_the_mixed_one_passes_through() {
    let mut warp = WarpJump::new();
    warp.warped(p(0, 500), PARK, 0);
    assert_eq!(
        warp.correct(-6, 1, 1),
        (-6, 1),
        "不像混着挪动距离：原样交出"
    );
    assert!(warp.is_pending(), "还在留意");
    assert_eq!(warp.correct(1440 - 4, 310, 5), (-4, 0));
}

#[test]
fn nothing_is_corrected_after_the_window_or_for_small_jumps() {
    let mut warp = WarpJump::new();
    warp.warped(p(0, 500), PARK, 0);
    assert_eq!(warp.correct(1440, 310, 300), (1440, 310), "过了 0.25 秒");
    assert!(!warp.is_pending());

    warp.warped(p(1436, 808), PARK, 0);
    assert!(!warp.is_pending(), "挪得很近：不处理");
    assert_eq!(warp.correct(4, 2, 1), (4, 2));
}

#[test]
fn only_one_movement_is_corrected() {
    let mut warp = WarpJump::new();
    warp.warped(p(0, 500), PARK, 0);
    assert_eq!(warp.correct(1440, 310, 1), (0, 0));
    assert_eq!(warp.correct(1440, 310, 2), (1440, 310), "第二次不再减");
}
