use std::path::Path;

use ferry_types::mark::{self, Piece, Sensitive};

#[test]
fn marks_round_trip_through_a_log_line() {
    let line = format!(
        "已连接 {} · 收到 {} 保存在 {}",
        mark::device("Mac Studio"),
        mark::file("证书.p12"),
        mark::path(Path::new("/Users/me/Downloads/证书.p12")),
    );
    assert_eq!(
        line,
        "已连接 ⟦d:Mac Studio⟧ · 收到 ⟦f:证书.p12⟧ 保存在 ⟦p:/Users/me/Downloads/证书.p12⟧"
    );
    assert_eq!(
        mark::pieces(&line),
        vec![
            Piece::Plain("已连接 "),
            Piece::Marked(Sensitive::Device, "Mac Studio"),
            Piece::Plain(" · 收到 "),
            Piece::Marked(Sensitive::File, "证书.p12"),
            Piece::Plain(" 保存在 "),
            Piece::Marked(Sensitive::Path, "/Users/me/Downloads/证书.p12"),
        ]
    );
    assert_eq!(
        mark::strip(&line),
        "已连接 Mac Studio · 收到 证书.p12 保存在 /Users/me/Downloads/证书.p12"
    );
}

#[test]
fn a_closing_bracket_inside_a_name_cannot_end_the_mark_early() {
    let line = mark::device("奇怪⟧的名字").to_string();
    assert_eq!(mark::strip(&line), "奇怪)的名字");
}

#[test]
fn broken_or_unknown_marks_stay_as_plain_text() {
    for line in [
        "⟦x:不认识⟧ 后面",
        "⟦d 没有冒号⟧",
        "⟦d:没有结尾",
        "开头 ⟦",
        "普通的一行",
    ] {
        assert_eq!(mark::strip(line), line, "{line}");
        assert!(
            mark::pieces(line)
                .iter()
                .all(|p| matches!(p, Piece::Plain(_))),
            "{line}"
        );
    }
}
