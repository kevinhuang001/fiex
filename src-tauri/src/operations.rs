use crate::files::validate_name;
use crate::ignores::IgnoreSet;
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

#[derive(Debug)]
enum Undo {
    Rename(PathBuf, PathBuf),
    Created(PathBuf),
}
#[derive(Default)]
pub struct Operations {
    undo: Mutex<Vec<Vec<Undo>>>,
}
#[derive(Serialize)]
pub struct OperationResult {
    pub paths: Vec<String>,
    pub errors: Vec<String>,
    pub skipped: usize,
}
fn exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}
pub fn unique(path: &Path) -> PathBuf {
    if !exists(path) {
        return path.into();
    }
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let ext = path
        .extension()
        .map(|s| format!(".{}", s.to_string_lossy()))
        .unwrap_or_default();
    for i in 2.. {
        let next = path.with_file_name(format!("{stem} ({i}){ext}"));
        if !exists(&next) {
            return next;
        }
    }
    unreachable!()
}
fn copy_tree(
    src: &Path,
    dest: &Path,
    ignore: &IgnoreSet,
    relative: &Path,
) -> Result<usize, String> {
    if ignore.is_ignored(relative) {
        return Ok(1);
    }
    let meta = fs::symlink_metadata(src).map_err(|e| e.to_string())?;
    if meta.file_type().is_symlink() {
        return Err("为避免复制到意外位置，请单独处理符号链接".into());
    }
    let mut skipped = 0;
    if meta.is_dir() {
        fs::create_dir(dest).map_err(|e| e.to_string())?;
        for item in fs::read_dir(src).map_err(|e| e.to_string())? {
            let item = item.map_err(|e| e.to_string())?;
            skipped += copy_tree(
                &item.path(),
                &dest.join(item.file_name()),
                ignore,
                &relative.join(item.file_name()),
            )?;
        }
    } else if meta.is_file() {
        let mut input = fs::File::open(src).map_err(|e| e.to_string())?;
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(dest)
            .map_err(|e| e.to_string())?;
        std::io::copy(&mut input, &mut output).map_err(|e| e.to_string())?;
        fs::set_permissions(dest, meta.permissions()).map_err(|e| e.to_string())?;
    } else {
        return Err("不能复制设备或特殊文件".into());
    }
    Ok(skipped)
}
impl Operations {
    pub fn record_created(&self, path: PathBuf) {
        self.undo.lock().unwrap().push(vec![Undo::Created(path)]);
    }
    pub fn clear_undo(&self) {
        self.undo.lock().unwrap().clear();
    }

    pub fn create(&self, dir: &str, name: &str, folder: bool) -> Result<String, String> {
        validate_name(name)?;
        let path = Path::new(dir).join(name);
        if folder {
            fs::create_dir(&path).map_err(|e| e.to_string())?;
        } else {
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map_err(|e| e.to_string())?;
        }
        self.undo
            .lock()
            .unwrap()
            .push(vec![Undo::Created(path.clone())]);
        Ok(path.to_string_lossy().into_owned())
    }
    pub fn rename(&self, source: &str, name: &str) -> Result<String, String> {
        validate_name(name)?;
        let src = Path::new(source);
        let dest = src.with_file_name(name);
        if src == dest {
            return Ok(source.into());
        }
        if exists(&dest) {
            return Err("同名项目已存在，请更换名称".into());
        }
        fs::rename(src, &dest).map_err(|e| e.to_string())?;
        self.undo
            .lock()
            .unwrap()
            .push(vec![Undo::Rename(dest.clone(), src.into())]);
        Ok(dest.to_string_lossy().into_owned())
    }
    pub fn transfer(
        &self,
        sources: &[String],
        destination: &str,
        moving: bool,
        keep_both: bool,
        ignores: &[String],
    ) -> Result<OperationResult, String> {
        let dest_dir = dunce::canonicalize(destination).map_err(|e| e.to_string())?;
        if !dest_dir.is_dir() {
            return Err("目标不是文件夹".into());
        }
        let ignore = IgnoreSet::new(ignores);
        let mut skipped = 0;
        // Validate every destination before any mutation.
        let mut plan = Vec::new();
        for source in sources {
            let src = PathBuf::from(source);
            let name = src.file_name().ok_or("不能操作根目录")?;
            if ignore.is_ignored(Path::new(name)) {
                skipped += 1;
                continue;
            }
            let real = dunce::canonicalize(&src).map_err(|e| e.to_string())?;
            if dest_dir.starts_with(&real) {
                return Err("不能把文件夹复制或移动到其自身内部".into());
            }
            let mut dest = dest_dir.join(name);
            if moving && dest == src {
                continue;
            }
            if exists(&dest) {
                if keep_both {
                    dest = unique(&dest);
                } else {
                    return Err(format!(
                        "{} 已存在。请选择“保留两者”或取消。",
                        dest.display()
                    ));
                }
            }
            if plan.iter().any(|(_, p): &(PathBuf, PathBuf)| p == &dest) {
                return Err("所选项目具有重复文件名，请分别操作".into());
            }
            plan.push((src, dest));
        }
        let mut undo = Vec::new();
        let mut paths = Vec::new();
        let mut errors = Vec::new();
        for (src, dest) in plan {
            let result: Result<usize, String> = if moving {
                fs::rename(&src, &dest)
                    .map(|_| 0)
                    .map_err(|e| format!("{e}。跨磁盘移动请先复制，核对后移到废纸篓。"))
            } else {
                copy_tree(
                    &src,
                    &dest,
                    &ignore,
                    Path::new(src.file_name().unwrap_or_default()),
                )
            };
            match result {
                Ok(skipped_children) => {
                    skipped += skipped_children;
                    paths.push(dest.to_string_lossy().into_owned());
                    undo.push(if moving {
                        Undo::Rename(dest, src)
                    } else {
                        Undo::Created(dest)
                    });
                }
                Err(e) => {
                    errors.push(format!("{}: {e}", src.display()));
                    if !moving && exists(&dest) {
                        undo.push(Undo::Created(dest));
                    }
                }
            }
        }
        if !undo.is_empty() {
            self.undo.lock().unwrap().push(undo);
        }
        Ok(OperationResult {
            paths,
            errors,
            skipped,
        })
    }
    pub fn undo(&self) -> Result<(), String> {
        let mut stack = self.undo.lock().unwrap();
        let mut actions = stack.pop().ok_or("没有可撤销的操作")?;
        while let Some(action) = actions.pop() {
            let result = match &action {
                Undo::Rename(src, dest) => {
                    if exists(dest) {
                        Err("原位置已有同名项目，无法撤销".into())
                    } else {
                        fs::rename(src, dest).map_err(|e| e.to_string())
                    }
                }
                // Put copies/new files in Trash on undo, preserving any intervening edits.
                Undo::Created(path) => trash::delete(path).map_err(|e| e.to_string()),
            };
            if let Err(e) = result {
                actions.push(action);
                stack.push(actions);
                return Err(e);
            }
        }
        Ok(())
    }
}
pub fn recycle(paths: &[String]) -> Result<OperationResult, String> {
    let mut result = OperationResult {
        paths: vec![],
        errors: vec![],
        skipped: 0,
    };
    for path in paths {
        match trash::delete(path) {
            Ok(()) => result.paths.push(path.clone()),
            Err(e) => result.errors.push(format!("{path}: {e}")),
        }
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transfers_never_overwrite() {
        let t = tempfile::tempdir().unwrap();
        let a = t.path().join("a");
        let b = t.path().join("b");
        fs::create_dir(&a).unwrap();
        fs::create_dir(&b).unwrap();
        fs::write(a.join("x"), "source").unwrap();
        fs::write(b.join("x"), "existing").unwrap();
        let op = Operations::default();
        assert!(op
            .transfer(
                &[a.join("x").to_string_lossy().into()],
                b.to_str().unwrap(),
                false,
                false,
                &[]
            )
            .is_err());
        op.transfer(
            &[a.join("x").to_string_lossy().into()],
            b.to_str().unwrap(),
            false,
            true,
            &[],
        )
        .unwrap();
        assert_eq!(fs::read_to_string(b.join("x")).unwrap(), "existing");
        assert!(b.join("x (2)").exists());
    }
    #[test]
    fn recursive_and_traversal_rejected() {
        let t = tempfile::tempdir().unwrap();
        let op = Operations::default();
        assert!(op
            .create(t.path().to_str().unwrap(), "../escape", true)
            .is_err());
        assert!(op
            .transfer(
                &[t.path().to_string_lossy().into()],
                t.path().to_str().unwrap(),
                false,
                true,
                &[]
            )
            .is_err());
    }
    #[test]
    fn rename_undo() {
        let t = tempfile::tempdir().unwrap();
        let op = Operations::default();
        fs::write(t.path().join("a"), "x").unwrap();
        op.rename(t.path().join("a").to_str().unwrap(), "b")
            .unwrap();
        op.undo().unwrap();
        assert!(t.path().join("a").exists());
    }
}
