use ezgui::{EventCtx, Key};

pub struct PerObjectActions {}

impl PerObjectActions {
    pub fn new() -> PerObjectActions {
        PerObjectActions {}
    }

    pub fn action<S: Into<String>>(&self, ctx: &mut EventCtx, key: Key, label: S) -> bool {
        ctx.input.contextual_action(key, label)
    }
}
