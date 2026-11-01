//! Connection-cache menu group. Remote selections only; the action set is
//! owned by the connection adapter and unchanged by capability gating.

use super::{
    builtin::Context,
    file_ops::{item, Selection},
};
use crate::plugins::native_invocation::NativeProviderInvocation;
use crate::plugins::resource::file_capabilities::FileCapabilities;
use fiex_provider_contracts::{
    ContextMenuProvider, MenuActivity, MenuActivityKind, MenuExecuteResult, MenuItem,
    ProviderError, ProviderInput, ProviderInvocation,
};
use serde_json::{json, Value};

pub(super) struct ConnectionCache;

impl ConnectionCache {
    fn available(&self, selection: &Selection<'_>, _: &FileCapabilities) -> bool {
        !selection.paths.is_empty()
            && selection
                .paths
                .iter()
                .all(|path| crate::remote::is_remote(path))
    }
    fn items(&self, selection: &Selection<'_>, _: &FileCapabilities) -> Vec<MenuItem> {
        let entries = selection.entries;
        let pinned = entries.iter().all(|entry| entry.pinned);
        let cached = entries.iter().all(|entry| entry.has_cache);
        let conflict = entries
            .iter()
            .any(|entry| entry.sync.as_deref() == Some("conflict"));
        let mut pin = if pinned {
            item("remote.unpin", "取消保留在本地", "pin-off", "")
        } else {
            item("remote.pin", "始终保留在本地", "cloud-download", "")
        };
        let label = crate::remote::connection_for_path(&selection.paths[0])
            .map(|(connection, _)| crate::i18n::t("{0} 触发同步").replace("{0}", &connection.name))
            .unwrap_or_else(|| crate::i18n::t("触发同步").into());
        if !pinned {
            activity(&mut pin, &label, MenuActivityKind::Sync, true);
        }
        let mut items = vec![pin];
        if !cached {
            let mut cache = item("remote.cache", "缓存到本地", "cloud-download", "");
            activity(
                &mut cache,
                crate::i18n::t("正在缓存…"),
                MenuActivityKind::Transfer,
                false,
            );
            items.push(cache);
        } else if !entries.iter().any(|entry| {
            entry.pinned || matches!(entry.sync.as_deref(), Some("conflict" | "pending"))
        }) {
            items.push(item("remote.uncache", "取消缓存", "cloud-off", ""));
        }
        if conflict {
            items.push(item(
                "remote.keepLocal",
                "用本地版本覆盖云端",
                "cloud-check",
                "",
            ));
            items.push(item(
                "remote.keepRemote",
                "保留本地备份并采用云端版本",
                "cloud-check",
                "",
            ));
        }
        items
    }
}

impl ContextMenuProvider for ConnectionCache {
    fn resolve(&self, invocation: &dyn ProviderInvocation) -> Result<Vec<MenuItem>, ProviderError> {
        if invocation.method() != "contextMenu.resolve" {
            return Err(ProviderError::method_not_found(invocation.method()));
        }
        let context = NativeProviderInvocation::local_data::<Context>(invocation)?;
        let selection = context.selection();
        let capabilities = context.capabilities();
        if self.available(&selection, &capabilities) {
            Ok(super::builtin::retain_unclaimed(
                context,
                self.items(&selection, &capabilities),
            ))
        } else {
            Ok(Vec::new())
        }
    }

    fn execute(
        &self,
        invocation: &dyn ProviderInvocation,
    ) -> Result<MenuExecuteResult, ProviderError> {
        if invocation.method() != "contextMenu.execute" {
            return Err(ProviderError::method_not_found(invocation.method()));
        }
        let context = NativeProviderInvocation::local_data::<Context>(invocation)?;
        let item_id = match invocation.input()? {
            ProviderInput::ContextMenuExecute(request) => request.item_id.as_deref(),
            _ => None,
        }
        .ok_or_else(|| ProviderError::invalid_params("menu execute requires an itemId"))?;
        let resolve = NativeProviderInvocation::context_menu_resolve(invocation)?
            .with_local_data(std::sync::Arc::new(context.clone()));
        if !ContextMenuProvider::resolve(self, &resolve)?
            .iter()
            .any(|item| item.id == item_id && !item.disabled)
        {
            return Err(ProviderError::invalid_params(
                "native menu action was not offered",
            ));
        }
        Ok(super::builtin::action_result(item_id, context))
    }
}

fn activity(item: &mut MenuItem, label: &str, kind: MenuActivityKind, quiet: bool) {
    item.activity = Some(MenuActivity {
        label: label.into(),
        kind,
        quiet,
    });
}
