use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn walk(
    conn: &StoredConnection,
    mount: &Mount,
    ignore: &IgnoreSet,
    local: &Path,
    dest_parent: &str,
    name: &str,
    existing: &HashMap<String, RemoteFile>,
    resolutions: &[Resolution],
    keep_both: bool,
    cancel: &AtomicBool,
    tasks: &mut Vec<FileTask>,
    dirs: &mut Vec<DirTask>,
    conflicts: &mut Vec<Conflict>,
    skipped: &mut usize,
    reserved: &mut HashSet<String>,
) -> Result<(), String> {
    check_cancelled(cancel)?;
    if ignore.is_ignored(Path::new(name)) {
        *skipped += 1;
        return Ok(());
    }
    let meta = fs::symlink_metadata(local).map_err(|error| error.to_string())?;
    if meta.file_type().is_symlink() || !(meta.is_file() || meta.is_dir()) {
        return Err(crate::i18n::t("暂不支持复制或移动符号链接及特殊文件").into());
    }
    let relation = child_path(dest_parent, name);
    let destination = navigation_path(conn, &relation)?;
    let existing = existing.get(name);
    let is_dir = meta.is_dir();
    let (source_size, source_modified, _) = version(local)?;

    if is_dir {
        if let Some(item) = existing {
            if !item.is_dir {
                let key = conflict_key(local, &destination, source_size, item.size);
                match action_for(&key, resolutions, keep_both) {
                    Some(Action::Skip) => {
                        *skipped += 1;
                        return Ok(());
                    }
                    Some(Action::KeepBoth) => {
                        let unique = unique_name(name, reserved);
                        reserved.insert(unique.clone());
                        let relation = child_path(dest_parent, &unique);
                        dirs.push(DirTask {
                            rel: relation.clone(),
                            name: unique,
                            existed: None,
                            node: None,
                        });
                        // Children of a freshly kept directory cannot collide.
                        copy_children(
                            conn,
                            mount,
                            ignore,
                            local,
                            &relation,
                            &HashMap::new(),
                            resolutions,
                            keep_both,
                            cancel,
                            tasks,
                            dirs,
                            conflicts,
                            skipped,
                            reserved,
                        )?;
                        return Ok(());
                    }
                    Some(Action::Replace) => {
                        dirs.push(DirTask {
                            rel: relation.clone(),
                            name: name.to_string(),
                            existed: Some(item.clone()),
                            node: None,
                        });
                        let children = list_remote(conn, &relation)?
                            .into_iter()
                            .map(|child| (child.name.clone(), child))
                            .collect();
                        copy_children(
                            conn,
                            mount,
                            ignore,
                            local,
                            &relation,
                            &children,
                            resolutions,
                            keep_both,
                            cancel,
                            tasks,
                            dirs,
                            conflicts,
                            skipped,
                            reserved,
                        )?;
                        return Ok(());
                    }
                    None => conflicts.push(Conflict {
                        key,
                        source: FileVersion {
                            path: local.to_string_lossy().into_owned(),
                            size: source_size,
                            modified: source_modified,
                            is_dir: true,
                        },
                        destination: FileVersion {
                            path: destination,
                            size: item.size,
                            modified: item.modified,
                            is_dir: item.is_dir,
                        },
                    }),
                }
                return Ok(());
            }
            // Existing directory: merge into it and remember its provider node
            // so files created below it receive the right parent.
            dirs.push(DirTask {
                rel: relation.clone(),
                name: name.to_string(),
                existed: None,
                node: Some(item.node_id.clone()),
            });
            let children = list_remote(conn, &relation)?
                .into_iter()
                .map(|child| (child.name.clone(), child))
                .collect();
            copy_children(
                conn,
                mount,
                ignore,
                local,
                &relation,
                &children,
                resolutions,
                keep_both,
                cancel,
                tasks,
                dirs,
                conflicts,
                skipped,
                reserved,
            )?;
            return Ok(());
        }
        dirs.push(DirTask {
            rel: relation.clone(),
            name: name.to_string(),
            existed: None,
            node: None,
        });
        copy_children(
            conn,
            mount,
            ignore,
            local,
            &relation,
            &HashMap::new(),
            resolutions,
            keep_both,
            cancel,
            tasks,
            dirs,
            conflicts,
            skipped,
            reserved,
        )?;
        return Ok(());
    }

    // A file.
    let Some(item) = existing else {
        tasks.push(FileTask {
            local: local.to_path_buf(),
            rel: relation,
            name: name.to_string(),
            size: source_size,
            replace: None,
        });
        return Ok(());
    };
    let key = conflict_key(local, &destination, source_size, item.size);
    match action_for(&key, resolutions, keep_both) {
        Some(Action::Skip) => *skipped += 1,
        Some(Action::KeepBoth) => {
            let unique = unique_name(name, reserved);
            reserved.insert(unique.clone());
            tasks.push(FileTask {
                local: local.to_path_buf(),
                rel: child_path(dest_parent, &unique),
                name: unique,
                size: source_size,
                replace: None,
            });
        }
        Some(Action::Replace) => tasks.push(FileTask {
            local: local.to_path_buf(),
            rel: relation,
            name: name.to_string(),
            size: source_size,
            replace: Some(item.clone()),
        }),
        None => conflicts.push(Conflict {
            key,
            source: FileVersion {
                path: local.to_string_lossy().into_owned(),
                size: source_size,
                modified: source_modified,
                is_dir: false,
            },
            destination: FileVersion {
                path: destination,
                size: item.size,
                modified: item.modified,
                is_dir: item.is_dir,
            },
        }),
    }
    Ok(())
}
