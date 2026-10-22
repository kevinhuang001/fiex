//! An OS copy drag can be handled as a move only by another Fiex window in
//! this process. External destinations always receive a copy-only URI drag.
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant},
};
#[derive(Default)]
pub struct DragSessions(Mutex<Option<Session>>);
struct Session {
    window: String,
    paths: Vec<PathBuf>,
    started: Instant,
    moving: bool,
    originals: Vec<String>,
}
#[derive(Clone, Serialize)]
pub struct DropPlan {
    pub paths: Vec<String>,
    pub moving: bool,
}
fn normalized(paths: &[String]) -> Result<Vec<PathBuf>, String> {
    let mut result = paths
        .iter()
        .map(|p| dunce::canonicalize(p).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    result.sort();
    result.dedup();
    Ok(result)
}
impl DragSessions {
    pub fn begin(
        &self,
        window: &str,
        originals: &[String],
        paths: &[String],
        moving: bool,
    ) -> Result<(), String> {
        if paths.is_empty() {
            return Err("请选择文件".into());
        }
        *self.0.lock().unwrap() = Some(Session {
            window: window.into(),
            paths: normalized(paths)?,
            started: Instant::now(),
            moving,
            originals: originals.to_vec(),
        });
        Ok(())
    }
    pub fn end(&self, window: &str) {
        let mut slot = self.0.lock().unwrap();
        if slot.as_ref().is_some_and(|s| s.window == window) {
            slot.take();
        }
    }
    pub fn resolve(&self, paths: &[String], consume: bool) -> Option<DropPlan> {
        let Ok(paths) = normalized(paths) else {
            return None;
        };
        let mut slot = self.0.lock().unwrap();
        let internal = slot
            .as_ref()
            .is_some_and(|s| s.paths == paths && s.started.elapsed() < Duration::from_secs(600));
        if !internal {
            return None;
        }
        let result = slot.as_ref().map(|s| DropPlan {
            paths: s.originals.clone(),
            moving: s.moving,
        });
        if consume {
            slot.take();
        }
        result
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_registered_matching_drag_is_internal_and_consumed_once() {
        let t = tempfile::tempdir().unwrap();
        let path = t.path().join("a");
        std::fs::write(&path, "a").unwrap();
        let paths = vec![path.to_string_lossy().into_owned()];
        let sessions = DragSessions::default();
        assert!(sessions.resolve(&paths, true).is_none());
        sessions.begin("source", &paths, &paths, true).unwrap();
        sessions.end("other");
        assert!(sessions
            .resolve(&[t.path().to_string_lossy().into_owned()], true)
            .is_none());
        assert!(sessions.resolve(&paths, false).unwrap().moving);
        assert!(sessions.resolve(&paths, true).unwrap().moving);
        assert!(sessions.resolve(&paths, true).is_none());
    }
    #[test]
    fn completed_or_cancelled_drag_cannot_turn_an_external_copy_into_a_move() {
        let t = tempfile::tempdir().unwrap();
        let file = t.path().join("export.txt");
        std::fs::write(&file, "data").unwrap();
        let exported = vec![file.to_string_lossy().into_owned()];
        let original = vec!["raw-volume/hello.txt".to_string()];
        let sessions = DragSessions::default();
        sessions
            .begin("source", &original, &exported, false)
            .unwrap();
        let captured = sessions.resolve(&exported, false).unwrap();
        assert_eq!(captured.paths, original);
        assert!(!captured.moving);
        sessions.end("source");
        assert!(sessions.resolve(&exported, true).is_none());
        // A receiving Fiex window retains only the provenance captured on enter.
        assert_eq!(captured.paths, original);
    }
}
