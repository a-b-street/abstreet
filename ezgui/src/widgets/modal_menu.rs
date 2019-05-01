use crate::widgets::{Menu, Position};
use crate::{EventCtx, GfxCtx, InputResult, Key, ScreenPt, Text};

pub struct NewModalMenu {
    menu: Menu<Key>,
    chosen_action: Option<String>,
}

impl NewModalMenu {
    pub fn new(prompt_line: &str, choices: Vec<(Key, &str)>, ctx: &EventCtx) -> NewModalMenu {
        let mut menu = Menu::new(
            Some(Text::prompt(prompt_line)),
            choices
                .iter()
                .map(|(key, action)| (Some(*key), action.to_string(), *key))
                .collect(),
            false,
            true,
            Position::TopRightOfScreen,
            ctx.canvas,
        );
        menu.mark_all_inactive();
        NewModalMenu {
            menu,
            chosen_action: None,
        }
    }

    pub fn get_bottom_left(&self, ctx: &EventCtx) -> ScreenPt {
        self.menu.get_bottom_left(ctx.canvas)
    }

    pub fn handle_event(&mut self, ctx: &mut EventCtx) {
        if let Some(ref action) = self.chosen_action {
            panic!("Caller didn't consume modal action '{}'", action);
        }

        match self.menu.event(ctx.input.event, ctx.canvas) {
            // TODO Only consume the input if it was a mouse on top of
            // the menu... because we don't want to also mouseover
            // stuff underneath
            // TODO Doesn't covered_areas handle this?
            InputResult::Canceled | InputResult::StillActive => {}
            InputResult::Done(action, _) => {
                assert!(!ctx.input.event_consumed);
                ctx.input.event_consumed = true;
                self.chosen_action = Some(action);
            }
        }
        self.menu.mark_all_inactive();
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
