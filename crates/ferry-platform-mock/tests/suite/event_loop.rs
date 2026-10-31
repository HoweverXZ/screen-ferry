use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use ferry_platform::EventLoop;
use ferry_platform_mock::MockEventLoop;

#[test]
fn tasks_from_other_threads_run_on_the_loop() {
    let event_loop = MockEventLoop::new();
    let dispatcher = event_loop.dispatcher();
    let count = Arc::new(AtomicU32::new(0));
    std::thread::scope(|s| {
        for _ in 0..3 {
            let (dispatcher, count) = (dispatcher.clone(), count.clone());
            s.spawn(move || {
                dispatcher.run_on_main(move || {
                    count.fetch_add(1, Ordering::SeqCst);
                })
            });
        }
    });
    assert_eq!(count.load(Ordering::SeqCst), 0, "排队，不立即执行");
    assert_eq!(event_loop.run_pending(), 3);
    assert_eq!(count.load(Ordering::SeqCst), 3);
}

#[test]
fn reopen_calls_the_handler() {
    let mut event_loop = MockEventLoop::new();
    let count = Arc::new(AtomicU32::new(0));
    let seen = count.clone();
    event_loop.on_reopen(Box::new(move || {
        seen.fetch_add(1, Ordering::SeqCst);
    }));
    event_loop.reopen();
    assert_eq!(count.load(Ordering::SeqCst), 1);
}
