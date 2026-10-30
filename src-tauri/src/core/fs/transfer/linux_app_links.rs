//! Executable symlinks import as ordinary .desktop launchers. The original
//! executable/link is retained; no escaping symlink enters the private store.
use super::*;
use crate::core::volumes::applications;
pub(super) fn import(
    paths: &[String],
    destination: &str,
    keep_both: bool,
    cancel: &AtomicBool,
) -> Result<Option<OperationResult>, String> {
    if paths.is_empty() || !(destination == applications::URI || applications::is_user(destination))
    {
        return Ok(None);
    }
    let links = paths
        .iter()
        .filter(|path| {
            !path.contains("://")
                && std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink())
        })
        .count();
    if links == 0 {
        return Ok(None);
    }
    if links != paths.len() {
        return Err(crate::i18n::t("请将可执行链接与其他文件分开导入").into());
    }
    let parent = binding::destination(destination, "core:application-import")?;
    let mut planned = Vec::new();
    for path in paths {
        check_cancelled(cancel)?;
        let source = binding::trusted(path, "core:application-import")?;
        source.require(FsOperation::Read, false)?;
        let metadata = source.transfer_metadata()?;
        parent.review_transfer(
            TransferDirection::Import,
            TransferAction::Copy,
            &metadata.name,
            &metadata,
            false,
        )?;
        source.review_transfer(
            TransferDirection::Export,
            TransferAction::Copy,
            &metadata.name,
            &metadata,
            false,
        )?;
        let executable = dunce::canonicalize(path).map_err(|e| e.to_string())?;
        let executable = executable
            .to_str()
            .filter(|s| !s.contains(['\n', '\r', '\0']))
            .ok_or("invalid executable path")?;
        let escaped = executable
            .replace('\\', r"\\\\")
            .replace('"', r#"\\\""#)
            .replace('`', r"\\`")
            .replace('$', r"\\$")
            .replace('%', "%%");
        let name = format!("{}.desktop", metadata.name);
        let mut target = binding::child(&parent, &name)?;
        if optional_stat(&target)?.is_some() {
            if !keep_both {
                return Err(crate::i18n::t("应用已存在，请重命名或选择保留两者").into());
            }
            target = unique(&parent, &name, false)?;
        }
        target.require(FsOperation::Write, true)?;
        let label = metadata.name.replace(['\n', '\r'], " ");
        let content = format!(
            "[Desktop Entry]\nType=Application\nName={label}\nExec=\"{escaped}\"\nTerminal=false\n"
        );
        planned.push((target, content));
    }
    let mut result = OperationResult::default();
    for (target, content) in planned {
        check_cancelled(cancel)?;
        let mut writer = target.adapter()?.begin_write(true)?;
        writer.write(content.as_bytes())?;
        writer.commit()?;
        crate::core::volumes::application_policy::finish_import(&target.path, true)?;
        super::super::changes::committed(&target.path);
        result.paths.push(target.path);
    }
    Ok(Some(result))
}
