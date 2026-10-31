use ferry_core::bounds;
use ferry_types::{Display, DisplayId, Rect};

fn display(bounds: Rect) -> Display {
    Display {
        id: DisplayId(0),
        bounds,
        scale_percent: 100,
        primary: false,
    }
}

#[test]
fn bounds_of_nothing_is_none() {
    assert_eq!(bounds(&[]), None);
}

#[test]
fn bounds_cover_every_display() {
    let displays = [
        display(Rect::new(0, 0, 2560, 1440)),
        display(Rect::new(-1920, 200, 1920, 1080)),
        display(Rect::new(2560, -300, 1080, 1920)),
    ];
    assert_eq!(bounds(&displays), Some(Rect::new(-1920, -300, 5560, 1920)));
}
