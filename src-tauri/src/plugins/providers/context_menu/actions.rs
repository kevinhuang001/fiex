use serde_json::Value;

pub async fn execute_remote(
    effect: &Value,
    options: &crate::remote::TransferOptions,
) -> Result<(), String> {
    let paths: Vec<String> =
        serde_json::from_value(effect["paths"].clone()).map_err(|error| error.to_string())?;
    match effect["command"].as_str() {
        Some("remote.pin") => crate::remote::sync(&paths, options).await,
        Some("remote.unpin") => crate::remote::unpin(&paths).await,
        Some("remote.uncache") => crate::remote::uncache(&paths).await,
        Some("remote.cache") => {
            for path in paths {
                if crate::remote::stat(&path)?.is_dir {
                    crate::remote::download_dir(&path, options).await?;
                } else {
                    crate::remote::download(&path, options).await?;
                }
            }
            Ok(())
        }
        Some("remote.keepLocal" | "remote.keepRemote") => {
            for path in paths {
                crate::remote::resolve(&path, effect["command"] == "remote.keepLocal").await?;
            }
            Ok(())
        }
        _ => Err("unknown remote menu operation".into()),
    }
}
