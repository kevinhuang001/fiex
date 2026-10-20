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
    ended: Option<Instant>,
    moving: bool,
    originals: Vec<String>,
}
#[derive(Serialize)]
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
            ended: None,
            moving,
            originals: originals.to_vec(),
        });
        Ok(())
    }
    pub fn end(&self, window: &str) {
        if let Some(s) = self
            .0
            .lock()
            .unwrap()
            .as_mut()
            .filter(|s| s.window == window)
        {
            s.ended = Some(Instant::now());
        }
    }
    pub fn take_internal(&self, paths: &[String]) -> Option<DropPlan> {
        let Ok(paths) = normalized(paths) else {
            return None;
        };
        let mut slot = self.0.lock().unwrap();
        let internal = slot.as_ref().is_some_and(|s| {
            s.paths == paths
                && s.started.elapsed() < Duration::from_secs(600)
                && s.ended
                    .is_none_or(|end| end.elapsed() < Duration::from_secs(3))
        });
        if internal {
            slot.take().map(|s| DropPlan {
                paths: s.originals,
                moving: s.moving,
            })
        } else {
            None
        }
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
        assert!(sessions.take_internal(&paths).is_none());
        sessions.begin("source", &paths, &paths, true).unwrap();
        sessions.end("other");
        assert!(sessions
            .take_internal(&[t.path().to_string_lossy().into_owned()])
            .is_none());
        sessions.end("source");
        assert!(sessions.take_internal(&paths).unwrap().moving);
        assert!(sessions.take_internal(&paths).is_none());
    }
}
