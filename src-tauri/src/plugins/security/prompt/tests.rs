use super::*;
use std::sync::{mpsc, Mutex, OnceLock};

fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

#[test]
fn concurrent_requests_share_one_prompt_and_a_denial_fails_the_batch() {
    let _guard = serial();
    let (sender, receiver) = mpsc::channel();
    set_sink(Box::new(move |event| {
        if let PermissionPromptEvent::Requested(request) = event {
            let _ = sender.send(request.id);
        }
    }));
    let plugin = format!("test.prompt.deny.{}", std::process::id());
    let mut threads = Vec::new();
    for _ in 0..4 {
        let plugin = plugin.clone();
        threads.push(std::thread::spawn(move || {
            request(
                &plugin,
                "files.read",
                PromptKind::Permission,
                None,
                Box::new(|| Ok(())),
            )
        }));
    }
    let id = receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("prompt emitted");
    std::thread::sleep(Duration::from_millis(100));
    assert!(
        receiver.try_recv().is_err(),
        "one prompt must cover the whole batch"
    );
    respond(&id, false).expect("denial resolved");
    for thread in threads {
        assert!(thread.join().unwrap().is_err());
    }
    // A retry inside the cooldown fails without opening another dialog.
    assert!(request(
        &plugin,
        "files.read",
        PromptKind::Permission,
        None,
        Box::new(|| Ok(())),
    )
    .is_err());
    assert!(receiver.try_recv().is_err());
}

#[test]
fn allowing_runs_the_grant_before_requesters_continue() {
    let _guard = serial();
    let (sender, receiver) = mpsc::channel();
    set_sink(Box::new(move |event| {
        if let PermissionPromptEvent::Requested(request) = event {
            let _ = sender.send(request.id);
        }
    }));
    let plugin = format!("test.prompt.allow.{}", std::process::id());
    let thread = {
        let plugin = plugin.clone();
        std::thread::spawn(move || {
            request(
                &plugin,
                "network",
                PromptKind::Network,
                Some("example.com"),
                Box::new(|| {
                    crate::plugins::security::permissions::normalize_domain("example.com")
                        .map(|_| ())
                }),
            )
        })
    };
    let id = receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("prompt emitted");
    respond(&id, true).expect("allow resolved");
    assert!(thread.join().unwrap().is_ok());
    respond(&id, true).expect_err("a resolved prompt cannot be answered twice");
}
