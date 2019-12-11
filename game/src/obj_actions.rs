use ezgui::{EventCtx, Key};
use std::cell::RefCell;

pub struct PerObjectActions {
    actions: RefCell<Vec<(Key, String)>>,
}

impl PerObjectActions {
    pub fn new() -> PerObjectActions {
        PerObjectActions {
            actions: RefCell::new(Vec::new()),
        }
    }

    // &self to avoid changing lots of code that previously took &UI
    pub fn action<S: Into<String>>(&self, ctx: &mut EventCtx, key: Key, label: S) -> bool {
        let lbl = label.into();
        self.actions.borrow_mut().push((key, lbl.clone()));
        ctx.input.contextual_action(key, lbl)
    }

    pub fn consume(&self) -> Vec<(Key, String)> {
        std::mem::replace(&mut self.actions.borrow_mut(), Vec::new())
    }

    pub fn reset(&mut self) {
        self.actions = RefCell::new(Vec::new());
    }
}
