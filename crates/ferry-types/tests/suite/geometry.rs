use ferry_types::{Point, Rect};

#[test]
fn contains_top_left_edge_but_not_bottom_right_edge() {
    let r = Rect::new(-10, 20, 100, 50);
    assert!(r.contains(Point::new(-10, 20)));
    assert!(r.contains(Point::new(89, 69)));
    assert!(!r.contains(Point::new(90, 20)));
    assert!(!r.contains(Point::new(-10, 70)));
    assert!(!r.contains(Point::new(-11, 20)));
}

#[test]
fn empty_rect_contains_nothing() {
    assert!(!Rect::new(0, 0, 0, 0).contains(Point::new(0, 0)));
}

#[test]
fn edges_saturate_instead_of_overflowing() {
    let r = Rect::new(i32::MAX - 1, i32::MAX - 1, 10, 10);
    assert_eq!(r.right(), i32::MAX);
    assert_eq!(r.bottom(), i32::MAX);
}
