use crate::widgets::{Menu, Position};
use crate::{EventCtx, GfxCtx, InputResult, MultiKey, ScreenPt, Slider, Text};

#[derive(Clone, Copy)]
pub enum SidebarPos {
    Left,
    Right,
    At(ScreenPt),
}

pub struct ModalMenu {
    menu: Menu<()>,
    chosen_action: Option<String>,
    choice_groups: Vec<Vec<(Option<MultiKey>, String, ())>>,
    pos: SidebarPos,
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
        let pos = SidebarPos::Right;
        let mut menu = Menu::new(
            Text::prompt(prompt_line),
            choice_groups.clone(),
            false,
            true,
            pos.pos(),
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
            pos,
        }
    }

    pub fn set_prompt(mut self, ctx: &mut EventCtx, prompt: Text) -> ModalMenu {
        self.menu.change_prompt(prompt, ctx.canvas);
        self
    }

    pub fn set_pos(mut self, ctx: &mut EventCtx, pos: SidebarPos) -> ModalMenu {
        self.pos = pos;
        self.rebuild_menu(ctx);
        self
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
            self.pos.pos(),
            ctx.canvas,
        );
        menu.mark_all_inactive();
        if ctx.canvas.hide_modal_menus {
            menu.make_hidden(ctx.canvas);
        }
        menu.change_prompt(self.menu.prompt.clone(), ctx.canvas);

        self.menu = menu;
    }

    pub fn get_total_width(&self) -> f64 {
        self.menu.get_total_width()
    }
}

impl SidebarPos {
    fn pos(&self) -> Position {
        match self {
            SidebarPos::Left => Position::TopLeftAt(ScreenPt::new(0.0, 0.0)),
            SidebarPos::Right => Position::TopRightOfScreen,
            SidebarPos::At(pt) => Position::TopLeftAt(*pt),
        }
    }

    // TODO Assumes the slider never moves
    pub fn below(slider: &Slider) -> SidebarPos {
        SidebarPos::At(slider.below_top_left())
    }
}
