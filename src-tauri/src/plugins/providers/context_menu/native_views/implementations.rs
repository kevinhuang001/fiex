//! Native view menus implement the shared ContextMenuProvider contract.
use super::NativeProviderInvocation;
use crate::core::files::Entry;
use crate::core::volumes::{applications, native_vfs, recycle, trash_vfs};
use fiex_provider_contracts::{
    ContextMenuProvider, MenuExecuteResult, MenuItem, MenuPlacement, ProviderError, ProviderInput,
    ProviderInvocation,
};

fn selection(ctx: &dyn ProviderInvocation) -> Result<(String, Vec<String>), ProviderError> {
    let directory = NativeProviderInvocation::directory(ctx)?.display.clone();
    let paths = (0..ctx.targets().len())
        .map(|index| {
            NativeProviderInvocation::resource(ctx, index).map(|resource| resource.display.clone())
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((directory, paths))
}

fn intersection(paths: &[String], mut items: impl FnMut(&str) -> Vec<MenuItem>) -> Vec<MenuItem> {
    let Some(first) = paths.first() else {
        return Vec::new();
    };
    items(first)
        .into_iter()
        .filter(|item| {
            paths
                .iter()
                .skip(1)
                .all(|path| items(path).iter().any(|other| other.id == item.id))
        })
        .collect()
}

fn offered<'a>(
    ctx: &'a dyn ProviderInvocation,
    items: &[MenuItem],
) -> Result<&'a str, ProviderError> {
    let id = match ctx.input()? {
        ProviderInput::ContextMenuExecute(request) => request.item_id.as_deref(),
        _ => None,
    }
    .ok_or_else(|| ProviderError::invalid_params("menu execute requires an itemId"))?;
    if items.iter().any(|item| item.id == id && !item.disabled) {
        Ok(id)
    } else {
        Err(ProviderError::invalid_params(
            "native menu action was not offered",
        ))
    }
}

fn item(id: &str, title: String, icon: &str, danger: bool, tail: bool) -> MenuItem {
    let mut item = MenuItem::action(id, title);
    item.icon = Some(icon.into());
    item.danger = danger;
    item.placement = tail.then_some(MenuPlacement::Tail);
    item
}

fn core_action(command: &str, paths: Vec<String>, directory: &str) -> MenuExecuteResult {
    let path = paths.first().cloned().unwrap_or_default();
    MenuExecuteResult::core_action(command, paths, path, directory)
}

pub(super) struct RecentMenu;
impl RecentMenu {
    fn items(path: &str) -> Vec<MenuItem> {
        let mut items = vec![item(
            "open",
            crate::i18n::t("打开").into(),
            "open",
            false,
            false,
        )];
        if native_vfs::recent_reveal_source(path).is_some() {
            items.push(item(
                "reveal",
                crate::i18n::t("打开文件所在位置").into(),
                "reveal",
                false,
                false,
            ));
        }
        items
    }
}

impl ContextMenuProvider for RecentMenu {
    fn resolve(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<MenuItem>, ProviderError> {
        let (_, paths) = selection(ctx)?;
        Ok(intersection(&paths, Self::items))
    }

    fn execute(&self, ctx: &dyn ProviderInvocation) -> Result<MenuExecuteResult, ProviderError> {
        let (directory, paths) = selection(ctx)?;
        let id = offered(ctx, &Self::resolve(self, ctx)?)?;
        match id {
            "open" => {
                let source_paths = paths
                    .iter()
                    .map(|path| crate::core::recent::source_for(path))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(ProviderError::internal)?;
                Ok(core_action("open", source_paths, &directory))
            }
            "reveal" => {
                for path in &paths {
                    native_vfs::recent_reveal_source(path).ok_or_else(|| {
                        ProviderError::internal(
                            crate::i18n::t("无法在文件管理器中显示该位置").to_string(),
                        )
                    })?;
                }
                Ok(core_action("reveal", paths, &directory))
            }
            _ => Err(ProviderError::invalid_params("unknown Recent menu action")),
        }
    }
}

pub(super) struct ApplicationsMenu;
impl ApplicationsMenu {
    fn items(path: &str) -> Vec<MenuItem> {
        let user = applications::is_user(path);
        let launch = path != applications::URI && (!user || user_application(path));
        let mut items = vec![item(
            "open",
            crate::i18n::t(if launch { "启动应用" } else { "打开" }).into(),
            "open",
            false,
            false,
        )];
        if user {
            items.push(item(
                "rename",
                crate::i18n::t("重命名").into(),
                "pencil",
                false,
                false,
            ));
            items.push(item(
                "delete",
                crate::i18n::t("彻底删除").into(),
                "trash",
                true,
                true,
            ));
        }
        items
    }
}

impl ContextMenuProvider for ApplicationsMenu {
    fn resolve(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<MenuItem>, ProviderError> {
        let (_, paths) = selection(ctx)?;
        Ok(intersection(&paths, Self::items))
    }

    fn execute(&self, ctx: &dyn ProviderInvocation) -> Result<MenuExecuteResult, ProviderError> {
        let (directory, paths) = selection(ctx)?;
        let id = offered(ctx, &Self::resolve(self, ctx)?)?;
        match id {
            "open" => Ok(core_action("open", paths, &directory)),
            "rename" | "delete" => {
                if paths.iter().any(|path| !applications::is_user(path)) {
                    return Err(ProviderError::invalid_params(
                        "only stored application entries can be modified",
                    ));
                }
                Ok(core_action(
                    if id == "rename" {
                        "rename"
                    } else {
                        "delete"
                    },
                    paths,
                    &directory,
                ))
            }
            _ => Err(ProviderError::invalid_params(
                "unknown Applications menu action",
            )),
        }
    }
}

fn user_application(path: &str) -> bool {
    applications::stored_entry(path).is_ok_and(|entry| user_application_entry(&entry))
}

fn user_application_entry(entry: &Entry) -> bool {
    !entry.is_dir || entry.extension == "app"
}

pub(super) struct TrashMenu;
impl TrashMenu {
    fn blank_items(directory: &str) -> Vec<MenuItem> {
        if directory == trash_vfs::URI || directory == recycle::URI {
            vec![item(
                "emptyTrash",
                crate::i18n::t(if cfg!(windows) {
                    "清空回收站"
                } else {
                    "清空废纸篓"
                })
                .into(),
                "trash",
                true,
                true,
            )]
        } else {
            Vec::new()
        }
    }

    fn items(_path: &str) -> Vec<MenuItem> {
        vec![
            item(
                "restore",
                crate::i18n::t("放回原处").into(),
                "restore",
                false,
                false,
            ),
            item(
                "delete",
                crate::i18n::t("彻底删除").into(),
                "trash",
                true,
                true,
            ),
        ]
    }
}

impl ContextMenuProvider for TrashMenu {
    fn resolve(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<MenuItem>, ProviderError> {
        let (directory, paths) = selection(ctx)?;
        if paths.is_empty() {
            return Ok(Self::blank_items(&directory));
        }
        let mut items = intersection(&paths, Self::items);
        if paths.len() > 1 {
            // Multi-selection can only permanently delete the exact offered
            // nodes, never empty the whole bin.
            items.retain(|item| item.id == "delete");
        }
        Ok(items)
    }

    fn execute(&self, ctx: &dyn ProviderInvocation) -> Result<MenuExecuteResult, ProviderError> {
        let (directory, paths) = selection(ctx)?;
        let id = offered(ctx, &Self::resolve(self, ctx)?)?;
        match id {
            "emptyTrash" if paths.is_empty() => Ok(core_action("emptyTrash", paths, &directory)),
            "restore" if paths.len() == 1 => Ok(core_action("restore", paths, &directory)),
            "delete" if !paths.is_empty() => Ok(core_action("delete", paths, &directory)),
            _ => Err(ProviderError::invalid_params("unknown Trash menu action")),
        }
    }
}
