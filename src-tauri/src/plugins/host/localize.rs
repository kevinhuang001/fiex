//! Localize declaration display fields without changing ids, paths or values.
use super::PluginManifest;
use serde_json::Value;
pub(super) fn manifest(manifest: PluginManifest) -> PluginManifest {
    manifest_for(manifest, crate::i18n::locale())
}
fn manifest_for(mut manifest: PluginManifest, locale: &str) -> PluginManifest {
    let catalog = manifest.locales.get(locale);
    let translate = |source: &str| {
        catalog
            .and_then(|messages| messages.get(source))
            .map(String::as_str)
            .unwrap_or(source)
            .to_string()
    };
    manifest.name = translate(&manifest.name);
    manifest.description = translate(&manifest.description);
    fn labels(value: &mut Value, translate: &impl Fn(&str) -> String) {
        match value {
            Value::Array(values) => values.iter_mut().for_each(|value| labels(value, translate)),
            Value::Object(values) => {
                for (key, value) in values {
                    if matches!(
                        key.as_str(),
                        "title" | "label" | "description" | "placeholder"
                    ) {
                        if let Some(source) = value.as_str() {
                            *value = Value::String(translate(source));
                        }
                    } else if key != "match" {
                        labels(value, translate);
                    }
                }
            }
            _ => {}
        }
    }
    labels(&mut manifest.extensions, &translate);
    manifest
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn localizes_labels_and_preserves_machine_and_user_values() {
        let source: PluginManifest = serde_json::from_value(json!({
            "schemaVersion":2,"id":"org.fiex.test","name":"工具","version":"1",
            "locales":{"en":{"工具":"Tools","名称":"Name","校验和":"Checksum","算法":"Algorithm"}},
            "extensions":{"settings":[{"id":"名称","label":"名称","defaultValue":"名称"}],"info":{
                "match":{"extensions":["名称"]},
                "sections":[{"id":"checksum","title":"校验和","fields":[
                    {"id":"sha256","label":"算法","compute":"onDemand"}
                ]}]
            }}
        }))
        .unwrap();
        let localized = manifest_for(source.clone(), "en");
        assert_eq!(localized.name, "Tools");
        assert_eq!(localized.extensions["settings"][0]["label"], "Name");
        assert_eq!(localized.extensions["settings"][0]["id"], "名称");
        assert_eq!(localized.extensions["settings"][0]["defaultValue"], "名称");
        assert_eq!(
            localized.extensions["info"]["sections"][0]["title"],
            "Checksum"
        );
        assert_eq!(
            localized.extensions["info"]["sections"][0]["fields"][0]["label"],
            "Algorithm"
        );
        assert_eq!(
            localized.extensions["info"]["match"],
            source.extensions["info"]["match"]
        );
        assert_eq!(manifest_for(source, "zh-CN").name, "工具");
    }
}
