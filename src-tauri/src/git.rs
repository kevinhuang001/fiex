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

#[derive(Serialize)]
pub struct GitRefs {
    pub current: String,
    pub branches: Vec<String>,
    pub tags: Vec<String>,
}

pub fn refs(root: &str) -> Result<GitRefs, String> {
    let current = output(Path::new(root), &["rev-parse", "--abbrev-ref", "HEAD"])
        .unwrap_or_else(|| "HEAD".into());
    let branches = output(
        Path::new(root),
        &["for-each-ref", "--format=%(refname:short)", "refs/heads"],
    )
    .unwrap_or_default()
    .lines()
    .map(str::to_string)
    .filter(|value| !value.is_empty())
    .collect::<Vec<_>>();
    let tags = output(
        Path::new(root),
        &["for-each-ref", "--format=%(refname:short)", "refs/tags"],
    )
    .unwrap_or_default()
    .lines()
    .map(str::to_string)
    .filter(|value| !value.is_empty())
    .collect::<Vec<_>>();
    Ok(GitRefs {
        current,
        branches,
        tags,
    })
}

pub fn checkout(root: &str, reference: &str) -> Result<(), String> {
    if reference.trim().is_empty() {
        return Err("请选择分支或版本".into());
    }
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["checkout", reference])
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().into())
    }
}
