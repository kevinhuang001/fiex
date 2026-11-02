use super::validation::*;
use super::{builtin, execute, query, query_core, query_plugins, sessions};
use serde_json::json;

mod capabilities;
mod contracts;
mod routing;

fn temp_selection() -> (tempfile::TempDir, String, Vec<String>) {
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path().to_string_lossy().into_owned();
    let paths = ["one.txt", "two.txt"]
        .map(|name| {
            let path = temp.path().join(name);
            std::fs::write(&path, "data").unwrap();
            path.to_string_lossy().into_owned()
        })
        .to_vec();
    (temp, directory, paths)
}
fn action<'a>(menus: &'a [serde_json::Value], id: &str) -> &'a serde_json::Value {
    menus
        .iter()
        .find(|menu| {
            menu["kind"] == "builtin"
                && menu["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|item| item["id"] == id)
        })
        .unwrap()
}

fn mount_entry(path: &str, is_dir: bool) -> crate::core::files::Entry {
    crate::core::files::Entry {
        path: path.into(),
        is_dir,
        ..Default::default()
    }
}

fn provider_ids(ctx: &super::builtin::Context) -> Vec<String> {
    super::builtin::providers(ctx)
        .into_iter()
        .map(|provider| provider.id)
        .collect()
}

fn mount_selection(
    path: &str,
    is_dir: bool,
    capabilities: crate::plugins::resource::file_capabilities::FileCapabilities,
) -> super::builtin::Context {
    super::builtin::Context::for_test_caps(
        path.rsplit_once('/').map_or(path, |(parent, _)| parent),
        vec![path.into()],
        vec![mount_entry(path, is_dir)],
        false,
        capabilities,
    )
}

fn emitted_items(ctx: &super::builtin::Context) -> Vec<serde_json::Value> {
    provider_ids(ctx)
        .iter()
        .filter_map(|id| super::builtin::resolve(id, ctx).ok())
        .flat_map(|menu| menu["items"].as_array().cloned().unwrap_or_default())
        .collect()
}

fn assert_never_disabled(ctx: &super::builtin::Context) {
    for item in emitted_items(ctx) {
        assert_eq!(item["disabled"], false, "builtin item was disabled: {item}");
    }
}
