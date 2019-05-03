use crate::widgets::{Menu, Position};
use crate::{Canvas, EventCtx, GfxCtx, InputResult, Key, ScreenPt, Text};

pub struct ModalMenu {
    menu: Menu<()>,
    chosen_action: Option<String>,
}

impl ModalMenu {
    pub fn new(prompt_line: &str, choices: Vec<(Option<Key>, &str)>, ctx: &EventCtx) -> ModalMenu {
        ModalMenu::hacky_new(prompt_line, choices, ctx.canvas)
    }

    // TODO Pass EventCtx when constructing the GUI?
    pub fn hacky_new(
        prompt_line: &str,
        choices: Vec<(Option<Key>, &str)>,
        canvas: &Canvas,
    ) -> ModalMenu {
        let mut menu = Menu::new(
            Text::prompt(prompt_line),
            choices
                .into_iter()
                .map(|(key, action)| (key, action.to_string(), ()))
                .collect(),
            false,
            true,
            Position::TopRightOfScreen,
            canvas,
        );
        menu.mark_all_inactive();
        if canvas.hide_modal_menus {
            menu.make_hidden(canvas);
        }
        ModalMenu {
            menu,
            chosen_action: None,
        }
    }

    pub fn get_bottom_left(&self) -> ScreenPt {
        self.menu.get_bottom_left()
    }

    pub fn handle_event(&mut self, ctx: &mut EventCtx, new_prompt: Option<Text>) {
        if let Some(ref action) = self.chosen_action {
            panic!("Caller didn't consume modal action '{}'", action);
        }

        match self.menu.event(ctx.input.event, ctx.canvas) {
            InputResult::Canceled | InputResult::StillActive => {}
            InputResult::Done(action, _) => {
                assert!(!ctx.input.event_consumed);
                ctx.input.event_consumed = true;
                self.chosen_action = Some(action);
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
