use crate::widgets::{Menu, Position};
use crate::{EventCtx, GfxCtx, InputResult, MultiKey, Text};

pub struct ModalMenu {
    menu: Menu<()>,
    chosen_action: Option<String>,
    choice_groups: Vec<Vec<(Option<MultiKey>, String, ())>>,
}

impl ModalMenu {
    pub fn new(
        prompt_line: &str,
        raw_choice_groups: Vec<Vec<(Option<MultiKey>, &str)>>,
        ctx: &EventCtx,
    ) -> ModalMenu {
        let choice_groups: Vec<Vec<(Option<MultiKey>, String, ())>> = raw_choice_groups
            .into_iter()
            .map(|group| {
                group
                    .into_iter()
                    .map(|(multikey, action)| (multikey, action.to_string(), ()))
                    .collect()
            })
            .collect();
        let mut menu = Menu::new(
            Text::prompt(prompt_line),
            choice_groups.clone(),
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
            choice_groups,
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

    pub fn set_prompt(mut self, ctx: &mut EventCtx, prompt: Text) -> ModalMenu {
        self.menu.change_prompt(prompt, ctx.canvas);
        self
    }

    pub fn add_action(&mut self, key: Option<MultiKey>, name: &str, ctx: &mut EventCtx) {
        self.choice_groups
            .last_mut()
            .unwrap()
            .push((key, name.to_string(), ()));
        self.rebuild_menu(ctx);
    }

    pub fn remove_action(&mut self, name: &str, ctx: &mut EventCtx) {
        self.choice_groups
            .last_mut()
            .unwrap()
            .retain(|(_, n, _)| n != name);
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

    pub fn action(&mut self, name: &str) -> bool {
        if let Some(ref action) = self.chosen_action {
            if name == action {
                self.chosen_action = None;
                return true;
            }
        } else {
            self.menu.mark_active(name, true);
        }
        false
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.menu.draw(g);
    }

    fn rebuild_menu(&mut self, ctx: &mut EventCtx) {
        let mut menu = Menu::new(
            Text::prompt(""),
            self.choice_groups.clone(),
            false,
            true,
            Position::TopRightOfScreen,
            ctx.canvas,
        );
        menu.mark_all_inactive();
        if ctx.canvas.hide_modal_menus {
            menu.make_hidden(ctx.canvas);
        }
        menu.change_prompt(self.menu.prompt.clone(), ctx.canvas);

        self.menu = menu;
    }
}
