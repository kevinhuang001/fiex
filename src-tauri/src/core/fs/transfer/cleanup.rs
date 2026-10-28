//! A move commits all copies before it considers deleting any source.
use super::*;
use std::{collections::BTreeSet, path::Path};

pub(super) fn validate(steps: &[Step], cancel: &AtomicBool) -> Result<(), String> {
    check_cancelled(cancel)?;
    for step in steps {
        step.source.require(FsOperation::Remove, false)?;
        if !same_version(&step.source.stat()?, &step.source_version) {
            return Err(format!(
                "Conflict: source changed before move cleanup: {}",
                step.source.path
            ));
        }
        if step.directory {
            let listed = step.source.list()?;
            let children = listed.as_array().ok_or("invalid source listing")?;
            let expected = steps
                .iter()
                .filter(|child| {
                    Path::new(&child.source.path).parent() == Some(Path::new(&step.source.path))
                })
                .filter_map(|child| Path::new(&child.source.path).file_name())
                .map(|name| name.to_string_lossy().into_owned())
                .collect::<BTreeSet<_>>();
            // Local adapters return scoped relative paths. Child names are
            // compared within their already bound parent, never across mounts.
            let actual = children
                .iter()
                .filter_map(|child| child["name"].as_str())
                .map(str::to_owned)
                .collect::<BTreeSet<_>>();
            if expected != actual || actual.len() != children.len() {
                return Err("Conflict: source directory changed before move cleanup".into());
            }
        }
    }
    Ok(())
}
