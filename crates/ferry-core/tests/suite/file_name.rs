use ferry_core::file_name::{MAX_DEPTH, MAX_NAME_BYTES, clean_name, clean_path, numbered, unique};

#[test]
fn ordinary_names_pass_through() {
    for name in [
        "报告.pdf",
        "设计稿",
        ".env",
        "a b",
        "照片 (1).png",
        "数据.tar.gz",
    ] {
        assert_eq!(clean_name(name), name);
    }
}

#[test]
fn characters_windows_rejects_become_underscores() {
    assert_eq!(clean_name("a:b?c*d"), "a_b_c_d");
    assert_eq!(clean_name("<x>|\"y\""), "_x___y_");
    assert_eq!(clean_name("tab\there"), "tab_here");
    assert_eq!(clean_name("a/b\\c"), "a_b_c");
}

#[test]
fn trailing_dots_and_spaces_are_dropped() {
    assert_eq!(clean_name("笔记. . "), "笔记");
    assert_eq!(clean_name("..."), "_");
    assert_eq!(clean_name(""), "_");
}

#[test]
fn reserved_device_names_are_prefixed() {
    assert_eq!(clean_name("CON"), "_CON");
    assert_eq!(clean_name("nul.txt"), "_nul.txt");
    assert_eq!(clean_name("Com3.log"), "_Com3.log");
    assert_eq!(
        clean_name("CONSOLE.txt"),
        "CONSOLE.txt",
        "只有完整的设备名才保留"
    );
}

#[test]
fn overlong_names_keep_their_extension() {
    let long = format!("{}.pdf", "文".repeat(200));
    let cleaned = clean_name(&long);
    assert!(cleaned.len() <= MAX_NAME_BYTES);
    assert!(cleaned.ends_with(".pdf"));
    assert!(cleaned.starts_with('文'), "在字符边界上截断");
}

#[test]
fn relative_paths_are_split_and_cleaned() {
    assert_eq!(
        clean_path("设计稿/首页:v2.png"),
        Some(vec!["设计稿".into(), "首页_v2.png".into()])
    );
    assert_eq!(clean_path("a"), Some(vec!["a".into()]));
}

#[test]
fn escaping_or_absolute_paths_are_rejected() {
    for bad in ["../etc/passwd", "a/../b", "/abs", "a//b", "a/./b", "a/", ""] {
        assert_eq!(clean_path(bad), None, "{bad:?}");
    }
    let deep = vec!["d"; MAX_DEPTH + 1].join("/");
    assert_eq!(clean_path(&deep), None);
    let ok = vec!["d"; MAX_DEPTH].join("/");
    assert!(clean_path(&ok).is_some());
}

#[test]
fn numbering_goes_before_the_extension() {
    assert_eq!(numbered("报告.pdf", 2), "报告 (2).pdf");
    assert_eq!(numbered("设计稿", 3), "设计稿 (3)");
    assert_eq!(numbered(".env", 2), ".env (2)");
    assert_eq!(numbered("a.tar.gz", 2), "a.tar (2).gz");
}

#[test]
fn unique_skips_names_already_taken() {
    let taken = ["报告.pdf", "报告 (2).pdf"];
    assert_eq!(unique("报告.pdf", |n| taken.contains(&n)), "报告 (3).pdf");
    assert_eq!(unique("新文件.txt", |n| taken.contains(&n)), "新文件.txt");
}
