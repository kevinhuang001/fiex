use serde::Serialize;
use std::{collections::HashMap, path::Path, process::Command};

#[derive(Serialize)]
pub struct GitStatus {
    pub root: String,
    pub branch: String,
    pub entries: HashMap<String, String>,
}

fn output(root: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Returns branch and per-file status when `git` is installed and `path` is in a
/// work tree. Status keys are absolute canonical paths so they match listings.
pub fn status(path: &str) -> Option<GitStatus> {
    if which::which("git").is_err() {
        return None;
    }
    let root = output(Path::new(path), &["rev-parse", "--show-toplevel"])?;
    let root = dunce::canonicalize(root).ok()?;
    let mut branch =
        output(&root, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_else(|| "HEAD".into());
    if branch == "HEAD" {
        branch = output(&root, &["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "HEAD".into());
    }
    let raw = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["status", "--porcelain=v1", "-z", "--untracked-files=all"])
        .output()
        .ok()?;
    if !raw.status.success() {
        return None;
    }
    let mut entries = HashMap::new();
    let mut fields = raw.stdout.split(|byte| *byte == 0);
    while let Some(field) = fields.next() {
        if field.len() < 4 {
            continue;
        }
        let xy = &field[..2];
        let relative = String::from_utf8_lossy(&field[3..]).replace('\\', "/");
        let code = String::from_utf8_lossy(xy).into_owned();
        let absolute = root.join(relative).to_string_lossy().into_owned();
        entries.insert(absolute, code);

        if xy.contains(&b'R') || xy.contains(&b'C') {
            // Rename/copy records are followed by the original path.
            let _ = fields.next();
        }
    }
    Some(GitStatus {
        root: root.to_string_lossy().into_owned(),
        branch,
        entries,
    })
}

fn run(root: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    Err(String::from_utf8_lossy(&output.stderr).trim().into())
}

fn relative_paths(root: &str, paths: &[String]) -> Result<Vec<String>, String> {
    let base = Path::new(root);
    paths
        .iter()
        .map(|path| {
            Path::new(path)
                .strip_prefix(base)
                .map(|relative| relative.to_string_lossy().into_owned())
                .map_err(|_| "路径不在 Git 仓库内".to_string())
        })
        .collect()
}

pub fn stage(root: &str, paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    let relative = relative_paths(root, paths)?;
    let mut args = vec!["add", "--"];
    args.extend(relative.iter().map(String::as_str));
    run(root, &args).map(|_| ())
}

pub fn unstage(root: &str, paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    let relative = relative_paths(root, paths)?;
    let mut args = vec!["restore", "--staged", "--"];
    args.extend(relative.iter().map(String::as_str));
    run(root, &args).map(|_| ())
}

pub fn commit(root: &str, message: &str) -> Result<(), String> {
    if message.trim().is_empty() {
        return Err("请输入提交信息".into());
    }
    run(root, &["commit", "-m", message]).map(|_| ())
}
