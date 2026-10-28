use super::*;

pub(super) fn join(dir: &str, name: &str) -> String {
    format!(
        "{}{}{}",
        dir.trim_end_matches(['/', '\\']),
        if dir.contains('\\') { "\\" } else { "/" },
        name
    )
}

pub(super) fn list(path: &str) -> Result<Vec<Entry>, String> {
    if let Some(r) = usbfs::route(path) {
        usbfs::listing(&r)
    } else {
        files::list(path, true, &[]).map(|l| l.entries)
    }
}

pub(super) fn info(path: &str) -> Result<Entry, String> {
    if let Some(r) = usbfs::route(path) {
        usbfs::entry(&r)
    } else {
        files::entry(Path::new(path))
    }
}

pub(super) fn parent(path: &str) -> Result<&str, String> {
    path.rsplit_once(['/', '\\'])
        .map(|(p, _)| p)
        .ok_or_else(|| crate::i18n::t("无效路径").into())
}

pub(super) fn read(path: &str) -> Result<Vec<u8>, String> {
    if let Some(r) = usbfs::route(path) {
        usbfs::read_path(&r)
    } else {
        fs::read(path).map_err(|e| e.to_string())
    }
}

pub(super) fn writable(path: &str) -> Result<(), String> {
    if usbfs::route(path).is_some_and(|r| {
        matches!(
            r.kind,
            usbfs::FsKind::Apfs | usbfs::FsKind::Hfs | usbfs::FsKind::Btrfs
        )
    }) {
        return Err(crate::i18n::t("此文件系统只读").into());
    }
    Ok(())
}

pub(super) fn create(path: &str, folder: bool) -> Result<(), String> {
    if let Some(r) = usbfs::route(parent(path)?) {
        usbfs::create(
            &r,
            Path::new(path)
                .file_name()
                .ok_or(crate::i18n::t("无效名称"))?
                .to_string_lossy()
                .as_ref(),
            folder,
        )
    } else if folder {
        fs::create_dir(path).map_err(|e| e.to_string())
    } else {
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

pub(super) fn write(path: &str, data: &[u8]) -> Result<(), String> {
    if let Some(r) = usbfs::route(path) {
        usbfs::write_path(&r, data)
    } else {
        fs::write(path, data).map_err(|e| e.to_string())
    }
}

pub(super) fn remove(path: &str) -> Result<(), String> {
    if info(path)?.is_dir {
        for child in list(path)? {
            remove(&child.path)?;
        }
    }
    if let Some(r) = usbfs::route(path) {
        usbfs::remove(&r)
    } else if info(path)?.is_dir {
        fs::remove_dir(path).map_err(|e| e.to_string())
    } else {
        fs::remove_file(path).map_err(|e| e.to_string())
    }
}

pub(super) fn rename(path: &str, name: &str) -> Result<String, String> {
    let to = join(parent(path)?, name);
    if let Some(r) = usbfs::route(path) {
        usbfs::rename(&r, name)?;
    } else {
        fs::rename(path, &to).map_err(|e| e.to_string())?;
    }
    Ok(to)
}

pub(super) fn version(e: &Entry) -> FileVersion {
    FileVersion {
        path: e.path.clone(),
        size: e.size,
        modified: e.modified,
        is_dir: e.is_dir,
    }
}
