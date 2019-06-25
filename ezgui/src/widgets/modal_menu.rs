use crate::widgets::{Menu, Position};
use crate::{EventCtx, GfxCtx, InputResult, MultiKey, Text};

pub struct ModalMenu {
    menu: Menu<()>,
    chosen_action: Option<String>,
}

impl ModalMenu {
    pub fn new(
        prompt_line: &str,
        choice_groups: Vec<Vec<(Option<MultiKey>, &str)>>,
        ctx: &EventCtx,
    ) -> ModalMenu {
        let mut menu = Menu::new(
            Text::prompt(prompt_line),
            choice_groups
                .into_iter()
                .map(|group| {
                    group
                        .into_iter()
                        .map(|(multikey, action)| (multikey, action.to_string(), ()))
                        .collect()
                })
                .collect(),
            false,
            true,
            Position::TopRightOfScreen,
            ctx.canvas,
        );
        menu.mark_all_inactive();
        if ctx.canvas.hide_modal_menus {
            menu.make_hidden(ctx.canvas);
        }
        ModalMenu {
            menu,
            chosen_action: None,
        }
    }

    pub fn handle_event(&mut self, ctx: &mut EventCtx, new_prompt: Option<Text>) {
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

        self.menu.mark_all_inactive();
        if let Some(txt) = new_prompt {
            self.menu.change_prompt(txt, ctx.canvas);
        }
    }

    pub fn action(&mut self, name: &str) -> bool {
        if let Some(ref action) = self.chosen_action {
            if name == action {
                self.chosen_action = None;
                return true;
            }
        } else {
            self.menu.mark_active(name);
        }
        false
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.menu.draw(g);
    }
}
