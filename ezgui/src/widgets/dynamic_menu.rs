use crate::widgets::{Menu, Position};
use crate::{EventCtx, GfxCtx, InputResult, MultiKey, Text};

// ModalMenu resets the state of the world every event. DynamicMenu is more traditional
// retained-mode style; choices are explicitly added and removed.
pub struct DynamicMenu {
    menu: Menu<()>,
    chosen_action: Option<String>,

    // This is also a prototype of maintaining a spec, then updating the underlying geometry and
    // mechanism when the spec changes.
    prompt: String,
    current_actions: Vec<(Option<MultiKey>, String)>,
}

impl DynamicMenu {
    pub fn new(prompt_line: &str, ctx: &EventCtx) -> DynamicMenu {
        let mut menu = Menu::new(
            Text::prompt(prompt_line),
            Vec::new(),
            false,
            true,
            Position::CenterLeft,
            ctx.canvas,
        );
        if ctx.canvas.hide_modal_menus {
            menu.make_hidden(ctx.canvas);
        }
        DynamicMenu {
            menu,
            chosen_action: None,
            prompt: prompt_line.to_string(),
            current_actions: Vec::new(),
        }
    }

    pub fn handle_event(&mut self, ctx: &mut EventCtx) {
        if let Some(ref action) = self.chosen_action {
            panic!("Caller didn't consume modal action '{}'", action);
        }

        // Example of a conflict is Escaping out of a context menu.
        if !ctx.input.event_consumed {
            match self.menu.event(ctx.input.event, ctx.canvas) {
                InputResult::Canceled | InputResult::StillActive => {}
                InputResult::Done(action, _) => {
                    ctx.input.event_consumed = true;
                    self.chosen_action = Some(action);
                }
            }
        }
    }

    pub fn action(&mut self, name: &str) -> bool {
        if let Some(ref action) = self.chosen_action {
            if name == action {
                self.chosen_action = None;
                return true;
            }
        }
        false
    }

    pub fn add_action(&mut self, maybe_key: Option<MultiKey>, name: &str, ctx: &mut EventCtx) {
        self.current_actions.push((maybe_key, name.to_string()));
        self.rebuild_menu(ctx);
    }

    pub fn remove_action(&mut self, name: &str, ctx: &mut EventCtx) {
        self.current_actions.retain(|(_, n)| name != n);
        self.rebuild_menu(ctx);
    }

    pub fn consume_action(&mut self, name: &str, ctx: &mut EventCtx) -> bool {
        if self.action(name) {
            self.remove_action(name, ctx);
            true
        } else {
            false
        }
    }

    fn rebuild_menu(&mut self, ctx: &mut EventCtx) {
        self.menu = Menu::new(
            Text::prompt(&self.prompt),
            if self.current_actions.is_empty() {
                Vec::new()
            } else {
                vec![self
                    .current_actions
                    .iter()
                    .map(|(mk, choice)| (*mk, choice.clone(), ()))
                    .collect()]
            },
            false,
            true,
            Position::CenterLeft,
            ctx.canvas,
        );
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.menu.draw(g);
    }
}
