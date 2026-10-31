use super::*;
#[derive(Clone)]
pub enum Handler {
    Builtin(&'static str),
    Plugin(Box<crate::plugins::host::PluginInfo>),
}
#[derive(Clone)]
pub struct Resolver {
    pub id: String,
    pub name: String,
    pub priority: i64,
    pub handler: Handler,
}
impl Resolver {
    pub fn builtin(id: &'static str, priority: i64) -> Self {
        Self {
            id: id.into(),
            name: "Fiex".into(),
            priority,
            handler: Handler::Builtin(id),
        }
    }
    pub fn json(&self) -> Value {
        json!({"id":self.id,"provider":self.name,"priority":self.priority})
    }
}
