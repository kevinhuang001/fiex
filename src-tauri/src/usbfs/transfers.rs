//! Transfers through raw filesystem drivers use the same conflict choices as
//! local files. All conflicts are collected before writes begin.

use crate::{
    core::files::{self, Entry},
    core::ignores::IgnoreSet,
    core::operations::{check_cancelled, OperationResult, CANCELLED},
    core::transfers::{Action, Conflict, FileVersion, Resolution},
    usbfs,
};
use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

mod io;
use io::*;

struct Task {
    source: Entry,
    target: String,
    existing: bool,
}

struct Plan<'a> {
    tasks: Vec<Task>,
    conflicts: Vec<Conflict>,
    dirs: Vec<(String, String)>,
    merges: Vec<String>,
    skipped: usize,
    ignores: IgnoreSet,
    resolutions: &'a [Resolution],
    keep_both: bool,
    moving: bool,
    cancel: &'a AtomicBool,
}

impl Plan<'_> {
    fn walk(
        &mut self,
        source: Entry,
        target: String,
        existing: Option<Entry>,
        relative: String,
    ) -> Result<(), String> {
        check_cancelled(self.cancel)?;
        if self.ignores.is_ignored(Path::new(&relative)) {
            self.skipped += 1;
            return Ok(());
        }
        if source.is_symlink {
            return Err(crate::i18n::t("暂不支持复制符号链接").into());
        }
        if self.moving {
            writable(&source.path)?;
        }
        if source.path == target && self.moving {
            return Ok(());
        }
        let mut target = target;
        let mut replace = false;
        if let Some(ref old) = existing {
            if source.is_dir && old.is_dir && source.path != target && !self.keep_both {
                self.merges.push(source.path.clone());
                let children = list(&target)?;
                for child in list(&source.path)? {
                    let found = children.iter().find(|e| e.name == child.name).cloned();
                    let to = join(&target, &child.name);
                    let rel = join(&relative, &child.name);
                    self.walk(child, to, found, rel)?;
                }
                return Ok(());
            }
            let key = format!(
                "{}\0{}\0{}\0{}\0{}\0{}",
                source.path, old.path, source.modified, old.modified, source.size, old.size
            );
            let action = if self.keep_both {
                Some(Action::KeepBoth)
            } else {
                self.resolutions
                    .iter()
                    .find(|r| r.key == key)
                    .map(|r| r.action)
            };
            match action {
                None => {
                    self.conflicts.push(Conflict {
                        key,
                        source: version(&source),
                        destination: version(old),
                    });
                    return Ok(());
                }
                Some(Action::Skip) => {
                    self.skipped += 1;
                    return Ok(());
                }
                Some(Action::Replace) => {
                    if source.path == target {
                        return Err(crate::i18n::t("不能替换自身，请选择保留两者").into());
                    }
                    replace = true;
                }
                Some(Action::KeepBoth) => {
                    let parent = parent(&target)?;
                    let siblings = list(parent)?;
                    let p = Path::new(&source.name);
                    let stem = if source.is_dir {
                        p.file_name()
                    } else {
                        p.file_stem()
                    }
                    .unwrap_or_default()
                    .to_string_lossy();
                    let extension = if source.is_dir {
                        String::new()
                    } else {
                        p.extension()
                            .map(|e| format!(".{}", e.to_string_lossy()))
                            .unwrap_or_default()
                    };
                    for n in 2.. {
                        let name = format!("{stem} ({n}){extension}");
                        if !siblings.iter().any(|e| e.name == name) {
                            target = join(parent, &name);
                            break;
                        }
                    }
                }
            }
        }
        if source.is_dir && !replace {
            self.dirs.push((source.path.clone(), target.clone()));
            for child in list(&source.path)? {
                let to = join(&target, &child.name);
                let rel = join(&relative, &child.name);
                self.walk(child, to, None, rel)?;
            }
        } else {
            self.tasks.push(Task {
                source,
                target,
                existing: replace,
            });
        }
        Ok(())
    }
}

fn copy_node(source: &Entry, target: &str, cancel: &AtomicBool) -> Result<(), String> {
    check_cancelled(cancel)?;
    if source.is_dir {
        create(target, true)?;
        for child in list(&source.path)? {
            copy_node(&child, &join(target, &child.name), cancel)?;
        }
    } else {
        let data = read(&source.path)?;
        check_cancelled(cancel)?;
        create(target, false)?;
        write(target, &data)?;
        if read(target)? != data {
            return Err(crate::i18n::t("写入后内容校验失败，源文件已保留").into());
        }
    }
    Ok(())
}

pub fn transfer(
    sources: &[String],
    destination: &str,
    moving: bool,
    keep_both: bool,
    ignores: &[String],
    resolutions: &[Resolution],
    cancel: &AtomicBool,
) -> Result<OperationResult, String> {
    check_cancelled(cancel)?;
    writable(destination)?;
    let children = list(destination)?;
    let mut plan = Plan {
        tasks: vec![],
        conflicts: vec![],
        dirs: vec![],
        merges: vec![],
        skipped: 0,
        ignores: IgnoreSet::new(ignores),
        resolutions,
        keep_both,
        moving,
        cancel,
    };
    for path in sources {
        check_cancelled(cancel)?;
        if destination == path
            || destination.starts_with(&format!("{path}/"))
            || destination.starts_with(&format!("{path}\\"))
        {
            return Err(crate::i18n::t("不能移动到自身内部").into());
        }
        let source = info(path)?;
        let existing = children.iter().find(|e| e.name == source.name).cloned();
        plan.walk(
            source.clone(),
            join(destination, &source.name),
            existing,
            source.name,
        )?;
    }
    let mut result = OperationResult {
        conflicts: plan.conflicts,
        skipped: plan.skipped,
        ..Default::default()
    };
    if !result.conflicts.is_empty() {
        return Ok(result);
    }
    for (_, to) in &plan.dirs {
        check_cancelled(cancel)?;
        create(to, true)?;
    }
    for (index, task) in plan.tasks.into_iter().enumerate() {
        check_cancelled(cancel)?;
        let backup = if task.existing {
            let name = format!(
                ".fiex-replaced-{}-{index}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            );
            Some(rename(&task.target, &name)?)
        } else {
            None
        };
        let result_copy = copy_node(&task.source, &task.target, cancel);
        match result_copy {
            Ok(()) => {
                if moving {
                    remove(&task.source.path)?;
                }
                if let Some(path) = backup {
                    remove(&path)?;
                }
                result.paths.push(task.target);
            }
            Err(error) => {
                if info(&task.target).is_ok() {
                    remove(&task.target)?;
                }
                if let Some(path) = backup {
                    rename(
                        &path,
                        Path::new(&task.target)
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .as_ref(),
                    )?;
                }
                if cancel.load(Ordering::Relaxed) {
                    return Err(CANCELLED.into());
                }
                result.errors.push(error);
            }
        }
    }
    if moving {
        for (src, _) in plan
            .dirs
            .into_iter()
            .rev()
            .chain(plan.merges.into_iter().map(|src| (src, String::new())))
        {
            if list(&src).is_ok_and(|v| v.is_empty()) {
                remove(&src)?;
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
