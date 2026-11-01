use std::path::Path;

use ferry_core::Redactor;
use ferry_types::mark;

#[test]
fn names_become_stable_codes() {
    let mut r = Redactor::new(None);
    r.local("Mac Studio");
    assert_eq!(r.device(&["BF-202610101159", "Windows"]), "设备 A");
    assert_eq!(r.device(&["MacBook"]), "设备 B");

    let line = format!(
        "{} 已连接 {}，本机给它起名 {}",
        mark::device("Mac Studio"),
        mark::device("BF-202610101159"),
        mark::device("Windows"),
    );
    assert_eq!(r.line(&line), "本机 已连接 设备 A，本机给它起名 设备 A");
    // 日志里遇到没登记过的设备，按出现顺序接着编。
    assert_eq!(r.line(&mark::device("会议室 PC").to_string()), "设备 C");
    assert_eq!(r.line(&mark::device("会议室 PC").to_string()), "设备 C");
}

#[test]
fn a_plain_word_that_happens_to_be_a_device_name_is_left_alone() {
    // 起名叫“Windows”的设备：只换标记里的，正文里的“Windows”不动。
    let mut r = Redactor::new(None);
    r.device(&["BF-1", "Windows"]);
    let line = format!("Windows 控制 Mac：{}", mark::device("Windows"));
    assert_eq!(r.line(&line), "Windows 控制 Mac：设备 A");
}

#[test]
fn file_names_keep_only_their_extension() {
    let mut r = Redactor::new(None);
    let line = format!(
        "发送 {} 和 {} 和 {} 再发一次 {}",
        mark::file("证书.p12"),
        mark::file("trace (2).jsonl"),
        mark::file("设计稿"),
        mark::file("证书.p12"),
    );
    assert_eq!(
        r.line(&line),
        "发送 文件 1.p12 和 文件 2.jsonl 和 文件 3 再发一次 文件 1.p12"
    );
}

#[test]
fn paths_lose_the_user_name_and_the_file_name() {
    let mut r = Redactor::new(Some("/Users/zhangsan"));
    let line = format!(
        "文件收好了 at={} · 找不到 /Users/zhangsan/Desktop/a.txt",
        mark::path(Path::new("/Users/zhangsan/Downloads/简历.pdf")),
    );
    assert_eq!(
        r.line(&line),
        "文件收好了 at=~/Downloads/文件 1.pdf · 找不到 ~/Desktop/a.txt"
    );

    let mut r = Redactor::new(Some(r"C:\Users\zhangsan"));
    let line = mark::path(Path::new(r"C:\Users\zhangsan\Downloads\报价单.xlsx")).to_string();
    assert_eq!(r.line(&line), r"~\Downloads\文件 1.xlsx");
}

#[test]
fn addresses_and_ordinary_text_are_kept() {
    let mut r = Redactor::new(Some("/Users/me"));
    let line = "连接中断：网络错误 · 192.168.2.18:65108";
    assert_eq!(r.line(line), line);
}

#[test]
fn known_names_are_also_replaced_in_unmarked_text() {
    let mut r = Redactor::new(None);
    r.local("Mac Studio");
    r.device(&["会议室 PC"]);
    r.device(&["BF-1", "Windows"]);
    let line = "引擎停止：局域网里有多台未配对的服务端：会议室 PC、Mac Studio；Windows 上没有权限";
    assert_eq!(
        r.line(line),
        "引擎停止：局域网里有多台未配对的服务端：设备 A、本机；Windows 上没有权限",
        "通用的名字（Windows）留着"
    );
}
