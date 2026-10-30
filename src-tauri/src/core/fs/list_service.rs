//! Singleflight for complete, unfiltered VFS listings. Authorization precedes joining.
use super::*;
use crate::core::files::Listing;
use std::{
    collections::HashMap,
    sync::{Arc, Condvar, Mutex, OnceLock},
};
type Flight = Arc<(Mutex<Option<Result<Listing, FsError>>>, Condvar)>;
fn flights() -> &'static Mutex<HashMap<String, Flight>> {
    static FLIGHTS: OnceLock<Mutex<HashMap<String, Flight>>> = OnceLock::new();
    FLIGHTS.get_or_init(Default::default)
}
pub(super) fn fresh(broker: &ResourceBroker) -> Result<Listing, FsError> {
    if !directory_cache::enabled(&broker.path) {
        return broker.adapter()?.desktop_listing(&broker.path, true, &[]);
    }
    let revision = super::changes::revision();
    let identity = directory_cache::key(&broker.path);
    let key = format!("{identity}:{revision}");
    let (flight, owner) = {
        let mut flights = flights().lock().unwrap_or_else(|e| e.into_inner());
        match flights.get(&key) {
            Some(flight) => (flight.clone(), false),
            None => {
                if flights.len() >= 1024 {
                    return Err(FsError::internal("too many concurrent directory listings"));
                }
                let flight = Arc::new((Mutex::new(None), Condvar::new()));
                flights.insert(key.clone(), flight.clone());
                (flight, true)
            }
        }
    };
    if owner {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let listing = broker.adapter()?.desktop_listing(&broker.path, true, &[])?;
            if revision == super::changes::revision()
                && identity == directory_cache::key(&broker.path)
            {
                directory_cache::store(&broker.path, &listing);
            }
            Ok(listing)
        }))
        .unwrap_or_else(|_| Err(FsError::from("VFS listing worker panicked")));
        *flight.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(result.clone());
        flight.1.notify_all();
        flights()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&key);
        result
    } else {
        let mut result = flight.0.lock().unwrap_or_else(|e| e.into_inner());
        while result.is_none() {
            result = flight.1.wait(result).unwrap_or_else(|e| e.into_inner());
        }
        result.as_ref().unwrap().clone()
    }
}
