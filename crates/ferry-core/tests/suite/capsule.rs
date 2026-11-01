use ferry_core::Side;
use ferry_core::capsule::{Action, Avatar, Capsule, Glyph, Look, SHADOW, Tile, hit, place};
use ferry_types::{Frame, Rgba, Shape};

const ACCENT: Rgba = Rgba::hex(0x0A6CFF, 255);
const SCREEN: Frame = Frame::new(0.0, 0.0, 1440.0, 900.0);

fn wide(action: Action, third: &str) -> Look {
    Look::Wide {
        avatar: Avatar::Device(Glyph::Monitor),
        lines: ["松手发送到".into(), "BF-202610101159".into(), third.into()],
        hot: true,
        offline: false,
        action,
    }
}

fn card() -> Look {
    Look::Card {
        from: "来自 MacBook".into(),
        name: "证书.p12".into(),
        detail: "已保存到「下载」· 3.2 KB".into(),
        tile: Tile::Doc {
            ext: "P12".into(),
            tone: Rgba::hex(0x0F766E, 255),
        },
        chip: false,
    }
}

/// 动画跑到停下来，最多 3 秒。
fn settle(capsule: &mut Capsule) {
    for _ in 0..180 {
        if !capsule.step(1.0 / 60.0) {
            return;
        }
    }
    panic!("3 秒内没有停下来");
}

fn texts(look: &Look) -> Vec<String> {
    let mut capsule = Capsule::new();
    capsule.show(
        look.clone(),
        Side::Left,
        place(look, Side::Left, SCREEN, 450.0, 1.0),
    );
    settle(&mut capsule);
    let (_, scene) = capsule.render(1.0, ACCENT).unwrap();
    scene
        .shapes
        .into_iter()
        .filter_map(|s| match s {
            Shape::Text { text, .. } => Some(text),
            _ => None,
        })
        .collect()
}

#[test]
fn each_look_has_the_size_of_the_prototype() {
    let tall = |k| Look::Tall {
        glyph: Glyph::Laptop,
        sending: true,
        offline: false,
        closeness: k,
    };
    assert_eq!(tall(0.0).size(), (34.0, 92.0));
    assert_eq!(tall(1.0).size(), (58.0, 168.0));
    assert_eq!(
        wide(Action::None, "证书.p12 · 3.2 KB").size(),
        (284.0, 82.0)
    );
    assert_eq!(
        wide(Action::Cancel, "").size(),
        (304.0, 82.0),
        "有按钮时更宽"
    );
    assert_eq!(wide(Action::None, "").size(), (236.0, 64.0), "两行字时更矮");
    assert_eq!(card().size(), (330.0, 120.0));
}

#[test]
fn the_capsule_docks_against_the_edge_facing_the_peer() {
    let look = wide(Action::None, "x");
    let left = place(&look, Side::Left, SCREEN, 450.0, 1.0);
    assert_eq!((left.x, left.y), (10.0, 450.0 - 41.0));
    let right = place(&look, Side::Right, SCREEN, 450.0, 1.0);
    assert_eq!(right.right(), 1430.0);
    // 靠近屏幕两端时不出界，并避开菜单栏和程序坞。
    let top = place(&look, Side::Left, SCREEN, 0.0, 1.0);
    assert!(top.y >= 34.0);
    let bottom = place(&look, Side::Left, SCREEN, 900.0, 1.0);
    assert!(bottom.bottom() <= 890.0);
    // 缩放 150% 时整体放大。
    let scaled = place(&look, Side::Left, SCREEN, 450.0, 1.5);
    assert_eq!((scaled.x, scaled.w, scaled.h), (15.0, 426.0, 123.0));
}

#[test]
fn a_tall_capsule_lies_down_on_the_top_and_bottom_edges() {
    let look = Look::Tall {
        glyph: Glyph::Monitor,
        sending: true,
        offline: false,
        closeness: 0.0,
    };
    let frame = place(&look, Side::Top, SCREEN, 720.0, 1.0);
    assert_eq!((frame.w, frame.h), (92.0, 34.0));
    assert_eq!(frame.x + frame.w / 2.0, 720.0);
}

#[test]
fn showing_grows_out_of_the_edge_and_settles_on_the_target() {
    let look = wide(Action::None, "证书.p12 · 3.2 KB");
    let target = place(&look, Side::Left, SCREEN, 450.0, 1.0);
    let mut capsule = Capsule::new();
    assert!(capsule.render(1.0, ACCENT).is_none(), "没显示时什么都不画");

    capsule.show(look, Side::Left, target);
    let first = capsule.frame();
    assert!(
        first.w < target.w && first.x <= target.x + 1.0,
        "从边缘上的小点长出来"
    );
    settle(&mut capsule);
    let done = capsule.frame();
    for (a, b) in [
        (done.x, target.x),
        (done.y, target.y),
        (done.w, target.w),
        (done.h, target.h),
    ] {
        assert!((a - b).abs() < 0.5, "{done:?} vs {target:?}");
    }

    let (window, scene) = capsule.render(1.0, ACCENT).unwrap();
    assert!((window.w - (target.w + 2.0 * SHADOW)).abs() < 0.5);
    assert!((scene.width - window.w).abs() < 0.01);
}

#[test]
fn moving_to_a_new_target_midway_does_not_jump() {
    let look = wide(Action::None, "a");
    let mut capsule = Capsule::new();
    capsule.show(
        look.clone(),
        Side::Left,
        place(&look, Side::Left, SCREEN, 300.0, 1.0),
    );
    settle(&mut capsule);
    let before = capsule.frame();
    capsule.show(
        look.clone(),
        Side::Left,
        place(&look, Side::Left, SCREEN, 600.0, 1.0),
    );
    capsule.step(1.0 / 60.0);
    let after = capsule.frame();
    assert!(
        (after.y - before.y).abs() < 40.0,
        "一帧只走一小段：{before:?} → {after:?}"
    );
}

#[test]
fn hiding_fades_out_and_then_draws_nothing() {
    let look = card();
    let mut capsule = Capsule::new();
    capsule.show(
        look.clone(),
        Side::Right,
        place(&look, Side::Right, SCREEN, 450.0, 1.0),
    );
    settle(&mut capsule);
    capsule.hide();
    assert!(capsule.is_visible(), "淡出过程中仍在屏幕上");
    settle(&mut capsule);
    assert!(!capsule.is_visible());
    assert!(capsule.render(1.0, ACCENT).is_none());
}

#[test]
fn buttons_can_be_clicked_only_once_the_content_shows() {
    let look = card();
    let mut capsule = Capsule::new();
    capsule.show(
        look.clone(),
        Side::Left,
        place(&look, Side::Left, SCREEN, 450.0, 1.0),
    );
    let (_, early) = capsule.render(1.0, ACCENT).unwrap();
    assert!(early.hits.is_empty(), "还在长出来时不能点");

    settle(&mut capsule);
    let (_, scene) = capsule.render(1.0, ACCENT).unwrap();
    let ids: Vec<u32> = scene.hits.iter().map(|h| h.id).collect();
    assert_eq!(ids, [hit::CLOSE, hit::OPEN, hit::REVEAL]);
    for h in &scene.hits {
        let inside =
            h.frame.x >= 0.0 && h.frame.right() <= scene.width && h.frame.bottom() <= scene.height;
        assert!(inside, "可点的地方在窗口里：{h:?}");
    }
}

#[test]
fn a_card_with_the_chip_can_be_dragged_from_the_chip_and_the_file_icon() {
    let Look::Card {
        from,
        name,
        detail,
        tile,
        ..
    } = card()
    else {
        unreachable!()
    };
    let look = Look::Card {
        from,
        name,
        detail,
        tile,
        chip: true,
    };
    let mut capsule = Capsule::new();
    capsule.show(
        look.clone(),
        Side::Left,
        place(&look, Side::Left, SCREEN, 450.0, 1.0),
    );
    settle(&mut capsule);
    let (_, scene) = capsule.render(1.0, ACCENT).unwrap();
    let clicks: Vec<u32> = scene
        .hits
        .iter()
        .filter(|h| !h.drag)
        .map(|h| h.id)
        .collect();
    assert_eq!(
        clicks,
        [hit::CLOSE, hit::OPEN, hit::REVEAL],
        "按钮仍是点一下"
    );
    let drags: Vec<_> = scene.hits.iter().filter(|h| h.drag).collect();
    assert_eq!(drags.len(), 2, "那一条和文件图标都能拖：{drags:?}");
    assert!(drags.iter().all(|h| h.id == hit::CHIP));
    let chip = drags
        .iter()
        .max_by(|a, b| a.frame.w.total_cmp(&b.frame.w))
        .unwrap();
    let open = scene.hits.iter().find(|h| h.id == hit::OPEN).unwrap();
    assert!(chip.frame.y >= open.frame.bottom(), "那一条在按钮下面");
    assert!(chip.frame.bottom() <= scene.height);
}

#[test]
fn the_lines_of_text_are_drawn() {
    assert_eq!(
        texts(&wide(Action::None, "证书.p12 · 3.2 KB")),
        ["松手发送到", "BF-202610101159", "证书.p12 · 3.2 KB"]
    );
    let progress = Look::Wide {
        avatar: Avatar::Ring(0.46),
        lines: [
            "发送中 46%".into(),
            "发布录屏.mov".into(),
            "593 MB / 1.2 GB".into(),
        ],
        hot: false,
        offline: false,
        action: Action::Cancel,
    };
    assert_eq!(texts(&progress)[0], "46%", "进度环中间写百分比");
    let card = texts(&card());
    assert!(card.contains(&"打开".to_owned()) && card.contains(&"在文件夹中显示".to_owned()));
}
