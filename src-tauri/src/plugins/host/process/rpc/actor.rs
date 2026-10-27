use super::Running;
use serde_json::Value;

#[derive(Clone)]
pub(super) struct Operation {
    pub(super) owner: String,
    pub(super) scope: Option<crate::plugins::resource::fs_access::Scope>,
}

pub(super) struct Actor {
    pub(super) owner: String,
    pub(super) scope: Option<crate::plugins::resource::fs_access::Scope>,
}

impl Running {
    pub(super) fn operation_actor(&self, p: &Value) -> Result<Operation, String> {
        let authorization = p
            .get("authorization")
            .ok_or("missing operation authorization")?;
        let id = authorization
            .get("operationId")
            .and_then(Value::as_u64)
            .ok_or("missing operation authorization")?;
        self.operations
            .lock()
            .map_err(|_| "operation registry unavailable")?
            .get(&id)
            .cloned()
            .ok_or_else(|| "operation authorization expired".into())
    }

    pub(super) fn actor(&self, p: &Value) -> Result<Actor, String> {
        let authorization = p.get("authorization").ok_or("missing authorization")?;
        if let Some(task) = authorization.get("taskId").and_then(Value::as_str) {
            let entry = self
                .tasks
                .lock()
                .map_err(|_| "task registry unavailable")?
                .get(task)
                .filter(|entry| matches!(entry.info.status.as_str(), "running" | "cancelling"))
                .cloned()
                .ok_or_else(|| "task authorization expired".to_string())?;
            return Ok(Actor {
                owner: entry.owner,
                scope: entry.scope,
            });
        }
        let id = authorization
            .get("operationId")
            .and_then(Value::as_u64)
            .ok_or("missing authorization")?;
        let operation = self
            .operations
            .lock()
            .map_err(|_| "operation registry unavailable")?
            .get(&id)
            .cloned()
            .ok_or_else(|| "operation authorization expired".to_string())?;
        Ok(Actor {
            owner: operation.owner,
            scope: operation.scope,
        })
    }
}
