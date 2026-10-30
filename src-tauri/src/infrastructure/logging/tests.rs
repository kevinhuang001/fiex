use super::*;
#[cfg(test)]
mod redaction {
    use super::clean;

    #[test]
    fn credential_headers_and_query_values_are_redacted() {
        let value = clean("Authorization: Bearer SECRET1 refresh_token=SECRET2", 1024);
        assert!(!value.contains("SECRET1"));
        assert!(!value.contains("SECRET2"));
        assert!(value.contains("[REDACTED]"));
        let json = clean(r#"{"access_token":"SECRET3"}"#, 1024);
        assert!(!json.contains("SECRET3"));
        let separated = clean("Authorization: Basic SECRET4", 1024);
        assert!(!separated.contains("SECRET4"));
    }
}

#[cfg(test)]
mod delivery_tests {
    use super::*;
    #[test]
    fn clearing_waits_for_queued_writes_and_history_never_resurrects() {
        let id = format!("test:log-clear:{}", now_ms());
        report(&id, "test", "Authorization: Bearer HIDDEN", None).unwrap();
        flush();
        assert!(list(&id).iter().all(|r| !r.message.contains("HIDDEN")));
        clear(&id);
        flush();
        assert!(list(&id).is_empty());
        report(&id, "test", "new entry", None).unwrap();
        assert_eq!(query(&id, 0, 500).entries.len(), 1);
        clear(&id);
    }
}
