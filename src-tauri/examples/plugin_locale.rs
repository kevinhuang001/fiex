//! Integration regression: locale belongs to the request, not business params.
use fiex_lib::{i18n, plugin_host, plugin_sandbox};
use serde_json::json;
fn main() {
    plugin_sandbox::run_wrapper_if_requested();
    let id = "org.fiex.mock";
    let (manifest, _) = plugin_host::installed_manifests()
        .into_iter()
        .find(|(manifest, _)| manifest.id == id)
        .expect("mock fixture");
    if !plugin_host::catalog()
        .iter()
        .any(|plugin| plugin.id == id && plugin.approved)
    {
        plugin_host::approve_install(id).unwrap();
    }
    plugin_host::activate(
        id,
        &plugin_host::activation_requirements(&manifest).unwrap(),
    )
    .unwrap();
    for locale in ["en", "zh-TW", "zh-CN"] {
        i18n::set_locale(locale).unwrap();
        for params in [
            json!({"locale":"business-value","path":"校验和.txt"}),
            json!(["中文.txt"]),
        ] {
            let response = plugin_host::global()
                .invoke(id, "mock.locale", params.clone())
                .unwrap();
            assert_eq!(response["locale"], locale);
            let mut echoed = response["params"].clone();
            if let Some(value) = echoed.as_object_mut() {
                value.remove("networkDomains");
            }
            assert_eq!(echoed, params);
        }
    }
    plugin_host::global().stop(id);
    println!("PASS: three locales, array/object params, data preserved");
}
