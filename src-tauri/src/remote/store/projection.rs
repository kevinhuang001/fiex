//! Public connection metadata and credential-redacted editing configuration.
use super::*;

pub(super) fn public(conn: &StoredConnection) -> ConnectionInfo {
    let builtin = terminal_connection(conn);
    if !builtin {
        let mount = mount_path(conn);
        let _ = fs::create_dir_all(&mount);
    }
    ConnectionInfo {
        id: conn.id.clone(),
        name: conn.name.clone(),
        kind: conn.kind.clone(),
        plugin_id: connection_plugin_id(conn),
        mount_path: if builtin {
            format!("fiex-terminal://{}", conn.id)
        } else {
            super::super::config::navigation_path(conn, "").unwrap_or_default()
        },
        saved: conn.saved,
        autostart: conn.autostart,
    }
}

pub fn list_connections() -> Vec<ConnectionInfo> {
    load_stored().iter().map(public).collect()
}

fn editable_config(mut connection: StoredConnection) -> ConnectionInput {
    for key in &connection.secret_keys {
        connection.settings.remove(key);
    }
    ConnectionInput {
        id: Some(connection.id),
        name: connection.name,
        kind: connection.kind,
        plugin_id: Some(connection.plugin_id),
        settings: connection.settings,
    }
}

pub fn connection_config(id: &str) -> Result<ConnectionInput, String> {
    load_stored()
        .into_iter()
        .find(|connection| connection.id == id)
        .map(editable_config)
        .ok_or_else(|| "connection not found".into())
}

#[cfg(test)]
mod edit_tests {
    use super::*;
    #[test]
    fn editing_exposes_configuration_but_never_credentials() {
        let connection: StoredConnection = serde_json::from_value(serde_json::json!({
            "id":"saved-ssh", "mountId":"stable", "name":"SSH", "kind":"org.fiex.ssh", "pluginId":"org.fiex.ssh",
            "settings":{"host":"example.com", "port":2222, "password":"private-value"},
            "secretKeys":["password"], "saved":true, "autostart":true
        })).unwrap();
        let config = serde_json::to_value(editable_config(connection)).unwrap();
        assert_eq!(config["id"], "saved-ssh");
        assert_eq!(config["settings"]["port"], 2222);
        assert!(config["settings"].get("password").is_none());
        assert!(!config.to_string().contains("private-value"));
    }
}
