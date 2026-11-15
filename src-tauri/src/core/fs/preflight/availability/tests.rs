use super::*;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
};

static CACHE_TEST_LOCK: Mutex<()> = Mutex::new(());

fn request(path: &str) -> PreflightRequest {
    PreflightRequest::new(
        PreflightPhase::Availability,
        PreflightOperation::CreateFile(CreateFilePreflight {
            parent_id: path.into(),
            name: None,
        }),
    )
}

#[test]
fn successful_availability_checks_are_cached() {
    let _guard = CACHE_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_availability_cache();
    let path = tempfile::tempdir()
        .unwrap()
        .path()
        .to_string_lossy()
        .into_owned();
    let request = request(&path);
    let calls = AtomicUsize::new(0);

    assert!(review_cached(&path, 0, &request, || {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }));
    assert!(review_cached(&path, 0, &request, || {
        calls.fetch_add(1, Ordering::SeqCst);
        Err(FsError::new(FsErrorKind::Offline, "must use cached result"))
    }));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    clear_availability_cache();
}

#[test]
fn transient_provider_failure_can_recover_on_the_next_check() {
    let _guard = CACHE_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_availability_cache();
    let path = tempfile::tempdir()
        .unwrap()
        .path()
        .to_string_lossy()
        .into_owned();
    let request = request(&path);
    let calls = AtomicUsize::new(0);

    assert!(!review_cached(&path, 0, &request, || {
        calls.fetch_add(1, Ordering::SeqCst);
        Err(FsError::new(FsErrorKind::Offline, "provider disconnected"))
    }));
    assert!(review_cached(&path, 0, &request, || {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }));
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    clear_availability_cache();
}

#[test]
fn transient_failure_wakes_single_flight_waiters_for_a_fresh_check() {
    let _guard = CACHE_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_availability_cache();
    let path = tempfile::tempdir()
        .unwrap()
        .path()
        .to_string_lossy()
        .into_owned();
    let request = request(&path);
    let key = AvailabilityCacheKey {
        path: path.clone(),
        resource_revision: 0,
        request: request.clone(),
    };
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let owner_path = path.clone();
    let owner_request = request.clone();

    let owner = thread::spawn(move || {
        review_cached(&owner_path, 0, &owner_request, || {
            entered_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(8)).unwrap();
            Err(FsError::new(
                FsErrorKind::Offline,
                "provider disconnected during the request",
            ))
        })
    });
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();

    let waiter_path = path.clone();
    let waiter_request = request.clone();
    let waiter = thread::spawn(move || review_cached(&waiter_path, 0, &waiter_request, || Ok(())));
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let waiting = availability_cache()
            .lock()
            .unwrap()
            .get_mut(&key)
            .is_some_and(|cell| Arc::strong_count(cell) == 3);
        if waiting {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "second caller did not join flight"
        );
        thread::sleep(Duration::from_millis(1));
    }

    release_tx.send(()).unwrap();
    assert!(!owner.join().unwrap());
    assert!(waiter.join().unwrap());
    clear_availability_cache();
}

#[test]
fn filesystem_change_notification_invalidates_cached_availability() {
    let _guard = CACHE_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_availability_cache();
    let path = tempfile::tempdir()
        .unwrap()
        .path()
        .to_string_lossy()
        .into_owned();
    let request = request(&path);
    let calls = AtomicUsize::new(0);

    assert!(!review_cached(&path, 0, &request, || {
        calls.fetch_add(1, Ordering::SeqCst);
        Err(FsError::new(FsErrorKind::Unsupported, "read only"))
    }));
    changes::removed_from(&path, &path);
    assert!(review_cached(&path, 0, &request, || {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }));
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    clear_availability_cache();
}

#[test]
fn registered_applications_provider_decision_flows_through_availability_cache() {
    let _guard = CACHE_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_availability_cache();

    let broker = crate::core::fs::binding::trusted(
        crate::core::volumes::applications::URI,
        "core:test-app-preflight-cache",
    )
    .unwrap();
    let name = format!(
        "fiex-preflight-{}.desktop",
        crate::plugins::mounts::new_id().unwrap()
    );
    let request = PreflightRequest::new(
        PreflightPhase::Availability,
        PreflightOperation::CreateFile(CreateFilePreflight {
            parent_id: crate::core::volumes::applications::URI.into(),
            name: Some(name.clone()),
        }),
    );

    // This crosses ResourceBroker -> ProviderRegistry -> Applications VFS;
    // the registered provider denies a name that Core has not reserved.
    assert!(!broker.review_availability_cached(&request));
    assert!(!broker.review_availability_cached(&request));
    assert_eq!(availability_cache().lock().unwrap().len(), 1);

    let reserved = crate::core::volumes::applications::reserve_child(
        crate::core::volumes::applications::URI,
        &name,
        false,
    )
    .unwrap();
    // This is the invalidation path used after a provider commit. It must
    // discard the cached denial so the same provider can observe the new state.
    changes::committed(&reserved);
    assert_eq!(availability_cache().lock().unwrap().len(), 0);
    assert!(broker.review_availability_cached(&request));

    crate::core::volumes::applications::cancel_reservation(&reserved);
    clear_availability_cache();
}
