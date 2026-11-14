use super::unique_name;
use crate::{ApplicationNode, ApplicationSource, ApplicationsState, ApplicationsStore, app_uri_in};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

impl ApplicationsStore {
    pub fn register_system_apps(
        &self,
        records: &[(String, String)],
    ) -> Result<std::collections::BTreeMap<String, String>, String> {
        let identities = records
            .iter()
            .map(|(locator, _)| (system_id(locator), locator.clone()))
            .collect::<Vec<_>>();
        let state = self.update(|state| {
            let incoming = identities
                .iter()
                .map(|(id, _)| id.as_str())
                .collect::<HashSet<_>>();
            let mut names = state
                .folders
                .values()
                .map(|folder| (folder.parent.clone(), folder.name.to_lowercase()))
                .chain(
                    state
                        .apps
                        .values()
                        .filter(|app| !incoming.contains(app.id.as_str()))
                        .map(|app| (app.parent.clone(), app.display_name.to_lowercase())),
                )
                .collect::<HashSet<_>>();
            for app in state.apps.values_mut() {
                if matches!(app.source, ApplicationSource::System { .. }) {
                    app.unavailable = true;
                }
            }
            for ((_, name), (id, locator)) in records.iter().zip(identities.iter()) {
                let parent = state.apps.get(id).and_then(|app| app.parent.clone());
                let unique_name = unique_name(&safe_name(name), &parent, &mut names);
                match state.apps.get_mut(id) {
                    Some(app) => {
                        app.display_name = unique_name;
                        app.unavailable = false;
                    }
                    None => {
                        let order = next_order(state, None);
                        state.apps.insert(
                            id.clone(),
                            ApplicationNode {
                                id: id.clone(),
                                parent: None,
                                display_name: unique_name,
                                source: ApplicationSource::System {
                                    stable_locator: locator.clone(),
                                },
                                order,
                                unavailable: false,
                            },
                        );
                    }
                }
            }
            Ok(())
        })?;
        Ok(identities
            .into_iter()
            .map(|(id, locator)| {
                let uri = app_uri_in(&state, &id).expect("system application was inserted");
                (locator, uri)
            })
            .collect())
    }
}

fn safe_name(value: &str) -> String {
    let name = value
        .chars()
        .map(|character| match character {
            '/' => '／',
            '\\' => '＼',
            character if character.is_control() => '�',
            character => character,
        })
        .collect::<String>();
    if name.trim().is_empty() {
        "Unnamed application".into()
    } else {
        name
    }
}

pub fn system_id(locator: &str) -> String {
    format!("sys-{}", hex::encode(Sha256::digest(locator.as_bytes())))
}

fn next_order(state: &ApplicationsState, parent: Option<&str>) -> i64 {
    state
        .folders
        .values()
        .filter(|folder| folder.parent.as_deref() == parent)
        .map(|folder| folder.order)
        .chain(
            state
                .apps
                .values()
                .filter(|app| app.parent.as_deref() == parent)
                .map(|app| app.order),
        )
        .max()
        .unwrap_or(-1)
        .saturating_add(1)
}

#[cfg(test)]
mod tests {
    use super::safe_name;

    #[test]
    fn system_application_names_are_safe_for_the_virtual_tree() {
        assert_eq!(safe_name("Editor/Viewer\\App"), "Editor／Viewer＼App");
        assert_eq!(safe_name("   "), "Unnamed application");
        assert_eq!(safe_name("Editor\u{0007}"), "Editor�");
    }
}
