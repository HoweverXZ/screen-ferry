use ferry_core::{EdgeSwitch, Side, Transition};
use ferry_types::{Point, Rect};

/// 本机 1000×800，远端 1920×1080；远端缩放 100%，位移一比一。
fn switch(side: Side) -> EdgeSwitch {
    EdgeSwitch::new(
        Rect::new(0, 0, 1000, 800),
        Rect::new(0, 0, 1920, 1080),
        side,
        100,
    )
}

fn enter(s: &mut EdgeSwitch, position: Point, dx: i32, dy: i32) -> Point {
    match s.local_motion(position, dx, dy) {
        Some(Transition::Enter { at, .. }) => at,
        other => panic!("应进入远端，实际 {other:?}"),
    }
}

#[test]
fn enters_only_when_pushing_past_the_edge() {
    let mut s = switch(Side::Right);
    assert_eq!(
        s.local_motion(Point::new(998, 400), 5, 0),
        None,
        "还没到边缘"
    );
    assert_eq!(
        s.local_motion(Point::new(999, 400), 0, 3),
        None,
        "在边缘但没有向外推"
    );
    assert_eq!(
        s.local_motion(Point::new(999, 400), -2, 0),
        None,
        "在边缘但向里走"
    );
    assert_eq!(
        s.local_motion(Point::new(999, 400), 4, 0),
        Some(Transition::Enter {
            seq: 1,
            at: Point::new(0, 540)
        }),
        "纵坐标按比例换算：400/800 → 540/1080",
    );
    assert!(s.is_remote());
    assert_eq!(
        s.local_motion(Point::new(999, 400), 4, 0),
        None,
        "已在远端，本机移动不再处理"
    );
}

#[test]
fn enters_on_every_side() {
    let cases = [
        (Side::Left, Point::new(0, 0), (-1, 0), Point::new(1919, 0)),
        (
            Side::Right,
            Point::new(999, 799),
            (1, 0),
            Point::new(0, 1078),
        ),
        (
            Side::Top,
            Point::new(500, 0),
            (0, -1),
            Point::new(960, 1079),
        ),
        (
            Side::Bottom,
            Point::new(500, 799),
            (0, 1),
            Point::new(960, 0),
        ),
    ];
    for (side, position, (dx, dy), expected) in cases {
        assert_eq!(
            enter(&mut switch(side), position, dx, dy),
            expected,
            "{side:?}"
        );
    }
}

#[test]
fn moves_inside_remote_and_stops_at_its_far_edges() {
    let mut s = switch(Side::Right);
    enter(&mut s, Point::new(999, 400), 1, 0);

    assert_eq!(
        s.remote_motion(30, -40, true),
        Some(Transition::Move(Point::new(30, 500)))
    );
    assert_eq!(
        s.remote_motion(5000, 0, true),
        Some(Transition::Move(Point::new(1919, 500)))
    );
    assert_eq!(
        s.remote_motion(10, 0, true),
        None,
        "已顶到远端右边，没有变化"
    );
    assert_eq!(
        s.remote_motion(0, -9000, true),
        Some(Transition::Move(Point::new(1919, 0)))
    );
    assert_eq!(s.remote_position(), Some(Point::new(1919, 0)));
}

#[test]
fn leaves_through_the_edge_facing_local() {
    let mut s = switch(Side::Right);
    enter(&mut s, Point::new(999, 400), 1, 0);
    s.remote_motion(10, 0, true);

    assert_eq!(
        s.remote_motion(-11, 0, true),
        Some(Transition::Leave {
            seq: 1,
            local_at: Point::new(999, 400)
        }),
        "纵坐标换算回本机：540/1080 → 400/800",
    );
    assert!(!s.is_remote());

    let at = enter(&mut s, Point::new(999, 0), 1, 0);
    assert_eq!(at, Point::new(0, 0));
    assert!(matches!(
        s.remote_motion(-1, 0, true),
        Some(Transition::Leave { seq: 2, .. })
    ));
}

#[test]
fn holding_a_button_keeps_the_cursor_on_remote() {
    let mut s = switch(Side::Right);
    enter(&mut s, Point::new(999, 400), 1, 0);
    assert_eq!(s.remote_motion(-50, 0, false), None, "停在边缘，位置不变");
    assert!(s.is_remote());
    assert!(matches!(
        s.remote_motion(-1, 0, true),
        Some(Transition::Leave { .. })
    ));
}

#[test]
fn scales_motion_and_carries_the_remainder() {
    let mut s = EdgeSwitch::new(
        Rect::new(0, 0, 1000, 800),
        Rect::new(0, 0, 3000, 2000),
        Side::Right,
        150,
    );
    enter(&mut s, Point::new(999, 0), 1, 0);

    assert_eq!(
        s.remote_motion(10, 0, true),
        Some(Transition::Move(Point::new(15, 0)))
    );
    assert_eq!(
        s.remote_motion(1, 0, true),
        Some(Transition::Move(Point::new(16, 0))),
        "1.5 → 1，余 0.5"
    );
    assert_eq!(
        s.remote_motion(1, 0, true),
        Some(Transition::Move(Point::new(18, 0))),
        "1.5 + 0.5 → 2"
    );
}

#[test]
fn abort_returns_control_to_local() {
    let mut s = switch(Side::Right);
    assert_eq!(s.abort(), None, "本来就在本机");

    enter(&mut s, Point::new(999, 400), 1, 0);
    assert_eq!(s.abort(), Some(Point::new(999, 400)));
    assert!(!s.is_remote());
    assert_eq!(s.remote_motion(5, 5, true), None);
}

#[test]
fn remote_layout_change_clamps_the_cursor() {
    let mut s = switch(Side::Right);
    enter(&mut s, Point::new(999, 799), 1, 0);
    s.remote_motion(1900, 0, true);

    s.set_remote(Rect::new(0, 0, 1280, 720), 100);
    assert_eq!(s.remote_position(), Some(Point::new(1279, 719)));
}

#[test]
fn never_enters_an_empty_remote() {
    let mut s = EdgeSwitch::new(
        Rect::new(0, 0, 1000, 800),
        Rect::new(0, 0, 0, 0),
        Side::Right,
        100,
    );
    assert_eq!(s.local_motion(Point::new(999, 0), 1, 0), None);
}

#[test]
fn leave_on_demand_reports_the_same_seq() {
    let mut s = switch(Side::Right);
    assert_eq!(s.leave(), None);

    enter(&mut s, Point::new(999, 400), 1, 0);
    assert_eq!(
        s.leave(),
        Some(Transition::Leave {
            seq: 1,
            local_at: Point::new(999, 400)
        })
    );
    assert!(!s.is_remote());
}
