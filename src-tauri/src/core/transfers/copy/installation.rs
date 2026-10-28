use super::*;

pub(crate) fn remove(path: &Path) -> Result<(), String> {
    if fs::symlink_metadata(path).is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink()) {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
    .map_err(|e| e.to_string())
}

pub(crate) fn install_copy(
    src: &Path,
    dest: &Path,
    ignore: &IgnoreSet,
    cancel: &AtomicBool,
    progress: Option<&Progress<'_>>,
) -> Result<usize, String> {
    check_cancelled(cancel)?;
    let staging = tempfile::Builder::new()
        .prefix(".fiex-copy-")
        .tempdir_in(dest.parent().ok_or(crate::i18n::t("目标路径无效"))?)
        .map_err(|e| e.to_string())?;
    let staged = staging.path().join("incoming");
    let report = |path: &Path| {
        if let Some(progress) = progress {
            progress.record(path);
        }
    };
    let skipped = copy_tree_with_progress(
        src,
        &staged,
        ignore,
        Path::new(src.file_name().unwrap_or_default()),
        cancel,
        Some(&report),
    )?;
    if staged.is_dir() {
        if exists(dest) {
            return Err(crate::i18n::t("目标已改变，请重试").into());
        }
        fs::rename(&staged, dest).map_err(|e| e.to_string())?;
    } else {
        match fs::hard_link(&staged, dest) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(crate::i18n::t("目标已改变，请重试").into())
            }
            Err(_) => {
                let mut output = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(dest)
                    .map_err(|e| e.to_string())?;
                let copied = fs::File::open(&staged)
                    .and_then(|mut input| {
                        copy_stream(&mut input, &mut output, cancel).map_err(std::io::Error::other)
                    })
                    .and_then(|_| fs::set_permissions(dest, fs::metadata(&staged)?.permissions()));
                drop(output);
                if let Err(e) = copied {
                    let _ = fs::remove_file(dest);
                    return Err(e.to_string());
                }
            }
        }
    }
    Ok(skipped)
}

pub(crate) fn move_tree(src: &Path, dest: &Path, cancel: &AtomicBool) -> Result<usize, String> {
    check_cancelled(cancel)?;
    if exists(dest) {
        return Err(crate::i18n::t("目标已改变，请重试").into());
    }
    if src.is_file() {
        match fs::hard_link(src, dest) {
            Ok(()) => {
                if let Err(e) = fs::remove_file(src) {
                    let _ = fs::remove_file(dest);
                    return Err(e.to_string());
                }
                return Ok(0);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(crate::i18n::t("目标已改变，请重试").into())
            }
            Err(_) => (),
        }
    } else {
        match fs::rename(src, dest) {
            Ok(()) => return Ok(0),
            Err(e) if e.kind() != std::io::ErrorKind::CrossesDevices => return Err(e.to_string()),
            Err(_) => (),
        }
    }
    install_copy(src, dest, &IgnoreSet::new(&[]), cancel, None)?;
    remove(src)?;
    Ok(0)
}
