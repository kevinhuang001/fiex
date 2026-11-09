//! Builtin menu policy. The frontend receives data, never provider closures.
use super::file_ops;
use crate::plugins::events::Resolver;
use fiex_provider_contracts::MenuExecuteResult;
#[cfg(test)]
use serde_json::Value;

mod context;
pub(super) use context::{Context, MenuPreflightAvailability};

/// All Core providers participate in one in-process resolver pass.
pub(super) fn providers(ctx: &Context) -> Vec<Resolver> {
    file_ops::providers(ctx)
}

#[cfg(test)]
pub(super) fn resolve(id: &str, ctx: &Context) -> Result<Value, String> {
    if super::native_views::owns(id) {
        return super::native_views::resolve(id, &ctx.directory, &ctx.paths);
    }
    file_ops::resolve(id, ctx)
}

#[cfg(test)]
pub(super) fn execute(id: &str, item_id: &str, ctx: &Context) -> Result<Value, String> {
    let offered = resolve(id, ctx)?;
    if !offered["items"].as_array().is_some_and(|items| {
        items
            .iter()
            .any(|item| item["id"] == item_id && item["disabled"] != true)
    }) {
        return Err("menu action is no longer available".into());
    }
    if super::native_views::owns(id) {
        return super::native_views::execute(id, item_id, &ctx.directory, &ctx.paths);
    }
    file_ops::execute(id, item_id, ctx)
}

pub(super) fn action_result(item_id: &str, ctx: &Context) -> MenuExecuteResult {
    let paths = if matches!(item_id, "remote.keepLocal" | "remote.keepRemote") {
        ctx.entries
            .iter()
            .filter(|entry| entry.sync.as_deref() == Some("conflict"))
            .map(|entry| entry.path.clone())
            .collect::<Vec<_>>()
    } else {
        ctx.paths.clone()
    };
    if item_id.starts_with("remote.") {
        MenuExecuteResult::remote_action(
            item_id,
            paths,
            ctx.paths.first().cloned().unwrap_or_default(),
            ctx.directory.clone(),
        )
    } else {
        MenuExecuteResult::core_action(
            item_id,
            paths,
            ctx.paths.first().cloned().unwrap_or_default(),
            ctx.directory.clone(),
        )
    }
}
