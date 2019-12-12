use ezgui::{hotkey, EventCtx, Key};
use std::cell::RefCell;

pub struct PerObjectActions {
    actions: RefCell<Vec<(Key, String)>>,
    chosen: RefCell<Option<String>>,
}

impl PerObjectActions {
    pub fn new() -> PerObjectActions {
        PerObjectActions {
            actions: RefCell::new(Vec::new()),
            chosen: RefCell::new(None),
        }
    }

    // &self to avoid changing lots of code that previously took &UI
    pub fn action<S: Into<String>>(&self, ctx: &mut EventCtx, key: Key, label: S) -> bool {
        let lbl = label.into();
        if self.chosen.borrow().as_ref() == Some(&lbl) {
            *self.chosen.borrow_mut() = None;
            return true;
        }

        self.actions.borrow_mut().push((key, lbl));
        ctx.input.new_was_pressed(hotkey(key).unwrap())
    }

    pub fn consume(&mut self) -> Vec<(Key, String)> {
        std::mem::replace(&mut self.actions.borrow_mut(), Vec::new())
    }

    pub fn reset(&mut self) {
        self.actions = RefCell::new(Vec::new());
    }

    pub fn action_chosen(&mut self, action: String) {
        let mut c = self.chosen.borrow_mut();
        assert!(c.is_none());
        *c = Some(action);
    }

    pub fn assert_chosen_used(&mut self) {
        if let Some(action) = &*self.chosen.borrow() {
            panic!("{} chosen, but nothing used it", action);
        }
    }

    pub fn get_active_keys(&self) -> Vec<Key> {
        let mut keys: Vec<Key> = self.actions.borrow().iter().map(|(k, _)| *k).collect();
        keys.sort();
        keys
    }
}
