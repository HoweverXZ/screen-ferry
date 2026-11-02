//! 用本机的一个小 HTTP 服务器测下载：整份下载、接着下、服务器不支持接着下、取消、超出大小。

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use ferry_net::{Error, Https};

/// 一个只会发同一份内容的 HTTP 服务器。`ranges` 为假时不理 Range，总是从头发。
struct Server {
    url: String,
    requests: Arc<Mutex<Vec<String>>>,
}

impl Server {
    fn start(body: Vec<u8>, ranges: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/file", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let seen = requests.clone();
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut range = None;
                let mut head = String::new();
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                        break;
                    }
                    if let Some(v) = line.to_ascii_lowercase().strip_prefix("range: bytes=") {
                        range = v.trim().trim_end_matches('-').parse::<usize>().ok();
                    }
                    head.push_str(&line);
                }
                seen.lock().unwrap().push(head);
                let (status, part) = match range {
                    Some(from) if ranges => ("206 Partial Content", &body[from..]),
                    _ => ("200 OK", &body[..]),
                };
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    part.len()
                );
                let _ = stream.write_all(part);
            }
        });
        Self { url, requests }
    }

    fn last_request(&self) -> String {
        self.requests
            .lock()
            .unwrap()
            .last()
            .cloned()
            .unwrap_or_default()
    }
}

fn body() -> Vec<u8> {
    (0..200_000u32).map(|i| (i % 251) as u8).collect()
}

fn sha256(data: &[u8]) -> String {
    ring::digest::digest(&ring::digest::SHA256, data)
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn temp_file(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ferry-https-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("download");
    let _ = std::fs::remove_file(&path);
    path
}

fn https() -> Https {
    Https::new(None, "ScreenFerry/test").unwrap()
}

#[test]
fn a_small_file_is_fetched_within_its_limit() {
    let server = Server::start(b"{\"schema\":1}".to_vec(), true);
    assert_eq!(https().fetch(&server.url, 1024).unwrap(), b"{\"schema\":1}");
    assert!(server.last_request().contains("ScreenFerry/test"));
    assert!(matches!(
        https().fetch(&server.url, 4),
        Err(Error::TooLarge)
    ));
}

#[test]
fn a_download_reports_its_size_and_hash() {
    let data = body();
    let server = Server::start(data.clone(), true);
    let path = temp_file("full");
    let mut last = (0, 0);
    let done = https()
        .download(
            &server.url,
            &path,
            data.len() as u64,
            &mut |d, t| last = (d, t),
            &AtomicBool::new(false),
        )
        .unwrap();
    assert_eq!(done.size, data.len() as u64);
    assert_eq!(done.sha256, sha256(&data));
    assert_eq!(last, (data.len() as u64, data.len() as u64));
    assert_eq!(std::fs::read(&path).unwrap(), data);
}

#[test]
fn a_partial_download_continues_where_it_stopped() {
    let data = body();
    let server = Server::start(data.clone(), true);
    let path = temp_file("resume");
    std::fs::write(&path, &data[..50_000]).unwrap();
    let done = https()
        .download(
            &server.url,
            &path,
            data.len() as u64,
            &mut |_, _| {},
            &AtomicBool::new(false),
        )
        .unwrap();
    assert!(
        server
            .last_request()
            .to_ascii_lowercase()
            .contains("range: bytes=50000-")
    );
    assert_eq!(done.sha256, sha256(&data));
    assert_eq!(std::fs::read(&path).unwrap(), data);
}

#[test]
fn a_server_without_ranges_sends_everything_again() {
    let data = body();
    let server = Server::start(data.clone(), false);
    let path = temp_file("restart");
    std::fs::write(&path, &data[..50_000]).unwrap();
    let done = https()
        .download(
            &server.url,
            &path,
            data.len() as u64,
            &mut |_, _| {},
            &AtomicBool::new(false),
        )
        .unwrap();
    assert_eq!(done.size, data.len() as u64);
    assert_eq!(done.sha256, sha256(&data));
}

#[test]
fn a_cancelled_download_stops_and_more_than_expected_is_refused() {
    let data = body();
    let server = Server::start(data.clone(), true);
    let path = temp_file("cancel");
    let cancel = AtomicBool::new(true);
    let result = https().download(
        &server.url,
        &path,
        data.len() as u64,
        &mut |_, _| {},
        &cancel,
    );
    assert!(matches!(result, Err(Error::Cancelled)));

    cancel.store(false, Ordering::Relaxed);
    let _ = std::fs::remove_file(&path);
    let result = https().download(&server.url, &path, 1_000, &mut |_, _| {}, &cancel);
    assert!(matches!(result, Err(Error::TooLarge)));
}
