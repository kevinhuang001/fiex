//! Recent's after-open hook and remove-from-Recent menu providers.
use super::{recent_record, store, RecentModuleServices, ROOT};
use fiex_provider_contracts::{
    ContextMenuProvider, HookProvider, HookResult, MenuExecuteResult, MenuItem, MenuPlacement,
    ProviderError, ProviderInput, ProviderInvocation,
};
use std::sync::Arc;

pub struct RecentHook {
    services: Arc<dyn RecentModuleServices>,
}

impl RecentHook {
    pub fn new(services: Arc<dyn RecentModuleServices>) -> Self {
        Self { services }
    }
}

impl HookProvider for RecentHook {
    fn invoke(&self, ctx: &dyn ProviderInvocation) -> Result<HookResult, ProviderError> {
        let request = ctx
            .input()?
            .hook_request()
            .ok_or_else(|| ProviderError::invalid_params("expected HookRequest"))?;
        if request.event != "afterOpen" {
            return Err(ProviderError::invalid_params(
                "unexpected Recent hook event",
            ));
        }
        let store = store(self.services.as_ref(), ctx)?;
        let candidate = self.services.resource_for_open(ctx)?;
        if let Some(record) = recent_record(candidate, &store, self.services.as_ref())
            .map_err(ProviderError::internal)?
        {
            store.upsert(record).map_err(ProviderError::internal)?;
        }
        Ok(HookResult { handled: false })
    }
}

pub struct RecentContextMenu {
    services: Arc<dyn RecentModuleServices>,
}

impl RecentContextMenu {
    pub fn new(services: Arc<dyn RecentModuleServices>) -> Self {
        Self { services }
    }

    fn paths(&self, ctx: &dyn ProviderInvocation) -> Result<(String, Vec<String>), ProviderError> {
        self.services.selection(ctx)
    }

    fn item(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<MenuItem>, ProviderError> {
        let (_, paths) = self.paths(ctx)?;
        if paths.is_empty() || paths.iter().any(|path| !path.starts_with(ROOT)) {
            return Ok(Vec::new());
        }
        let zh = ctx.locale_tag().starts_with("zh");
        let mut item = MenuItem::action(
            "removeFromRecent",
            if paths.len() == 1 {
                if zh {
                    "从最近使用中移除"
                } else {
                    "Remove from Recent"
                }
            } else if zh {
                "从最近使用中移除选中项"
            } else {
                "Remove selected from Recent"
            },
        );
        item.icon = Some("trash".into());
        item.danger = true;
        item.placement = Some(MenuPlacement::Tail);
        Ok(vec![item])
    }
}

impl ContextMenuProvider for RecentContextMenu {
    fn resolve(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<MenuItem>, ProviderError> {
        if ctx.method() != "contextMenu.resolve" {
            return Err(ProviderError::method_not_found(ctx.method()));
        }
        self.item(ctx)
    }

    fn execute(&self, ctx: &dyn ProviderInvocation) -> Result<MenuExecuteResult, ProviderError> {
        if ctx.method() != "contextMenu.execute" {
            return Err(ProviderError::method_not_found(ctx.method()));
        }
        let (directory, paths) = self.paths(ctx)?;
        let id = match ctx.input()? {
            ProviderInput::ContextMenuExecute(request) => request.item_id.as_deref(),
            _ => None,
        }
        .ok_or_else(|| ProviderError::invalid_params("menu execute requires an itemId"))?;
        if id != "removeFromRecent"
            || paths.is_empty()
            || paths.iter().any(|path| !path.starts_with(ROOT))
        {
            return Err(ProviderError::invalid_params(
                "remove from Recent requires an offered Recent selection",
            ));
        }
        let store = store(self.services.as_ref(), ctx)?;
        let removed = store
            .remove_aliases(&paths)
            .map_err(ProviderError::internal)?;
        self.services.invalidate_removed_aliases(&removed);
        let path = paths[0].clone();
        Ok(MenuExecuteResult::core_action(
            "removeFromRecent",
            paths,
            path,
            directory,
        ))
    }
}
