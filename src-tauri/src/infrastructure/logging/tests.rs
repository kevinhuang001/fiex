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

#[cfg(test)]
mod detail_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn request_detail_keeps_important_fields_and_drops_secrets() {
        let detail = request_detail(
            "vfs.search",
            &json!({
                "nodeId": "node-1",
                "query": "report",
                "mode": "name",
                "hidden": true,
                "limit": 200,
                "ignores": ["a", "b"],
                "token": "SECRET",
                "dataBase64": "U0VDUkVU"
            }),
        );
        assert!(detail.contains("nodeId=\"node-1\""), "{detail}");
        assert!(detail.contains("query=\"report\""), "{detail}");
        assert!(detail.contains("mode=\"name\""), "{detail}");
        assert!(detail.contains("ignores=[2 items]"), "{detail}");
        assert!(!detail.contains("SECRET"), "{detail}");
        assert!(!detail.contains("dataBase64"), "{detail}");
    }

    #[test]
    fn response_detail_reports_counts_without_payloads() {
        let detail = response_detail(&json!({
            "entries": [{"name": "a"}, {"name": "b"}],
            "truncated": true,
            "nextCursor": "2"
        }));
        assert!(detail.contains("entries=2"), "{detail}");
        assert!(detail.contains("truncated=true"), "{detail}");
        assert!(detail.contains("nextCursor=yes"), "{detail}");
        assert!(!detail.contains("\"a\""), "{detail}");
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;

    #[test]
    fn sdk_request_lifecycle_duplicates_are_recognized() {
        assert!(is_sdk_request_lifecycle(
            "vfs.list completed; duration_ms=12"
        ));
        assert!(is_sdk_request_lifecycle(
            "git.status failed (RPC -32603); duration_ms=5"
        ));
        assert!(is_sdk_request_lifecycle(
            "vfs.search expected outcome (-32601); duration_ms=1"
        ));
        assert!(!is_sdk_request_lifecycle("background task started"));
        assert!(!is_sdk_request_lifecycle("Sync completed; duration_ms=9"));
    }
}
