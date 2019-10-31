use crate::layout::{ContainerOrientation, Widget};
use crate::widgets::PopupMenu;
use crate::{
    layout, Button, Choice, EventCtx, GfxCtx, InputResult, MultiKey, ScreenDims, ScreenPt, Text,
};

// TODO Ideally:
// - Pause sim while this is active
// - Grey out inactive items like ModalMenu
// Right now the uses of this don't really need this.
pub struct MenuUnderButton {
    button: Button,
    menu: PopupMenu<()>,
    expanded: bool,
    chosen_action: Option<String>,
    // TODO Hackish. While unexpanded.
    unexpanded_choices: Vec<(MultiKey, String)>,
    standalone_layout: ContainerOrientation,
}

impl MenuUnderButton {
    pub fn new(
        icon: &str,
        title: &str,
        choices: Vec<(Option<MultiKey>, &str)>,
        percent_along_top_of_screen: f64,
        ctx: &EventCtx,
    ) -> MenuUnderButton {
        MenuUnderButton {
            button: Button::icon_btn(icon, 32.0, title, None, ctx),
            menu: PopupMenu::new(
                Text::prompt(title),
                choices
                    .iter()
                    .map(|(mk, name)| Choice::new(*name, ()).multikey(*mk))
                    .collect(),
                ctx,
            )
            .disable_standalone_layout(),
            expanded: false,
            chosen_action: None,
            unexpanded_choices: choices
                .into_iter()
                .filter_map(|(mk, name)| {
                    if let Some(key) = mk {
                        Some((key, name.to_string()))
                    } else {
                        None
                    }
                })
                .collect(),
            standalone_layout: ContainerOrientation::Top(percent_along_top_of_screen),
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx) {
        if let Some(ref c) = self.chosen_action {
            panic!("Nothing consumed action {}", c);
        }

        layout::stack_vertically(self.standalone_layout, ctx.canvas, vec![self]);

        self.button.event(ctx);
        if self.button.clicked() {
            self.expanded = !self.expanded;
            return;
        }

        if self.expanded {
            match self.menu.event(ctx) {
                InputResult::StillActive => {}
                InputResult::Done(name, _) => {
                    self.chosen_action = Some(name);
                    self.expanded = false;
                }
                InputResult::Canceled => {
                    self.expanded = false;
                }
            }
        } else {
            for (mk, name) in &self.unexpanded_choices {
                if ctx.input.new_was_pressed(*mk) {
                    self.chosen_action = Some(name.to_string());
                    break;
                }
            }
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.button.draw(g);
        if self.expanded {
            self.menu.draw(g);
        }
    }

    pub fn action(&mut self, label: &str) -> bool {
        if let Some(ref action) = self.chosen_action {
            if label == action {
                self.chosen_action = None;
                return true;
            }
        }
        false
    }
}

impl Widget for MenuUnderButton {
    fn get_dims(&self) -> ScreenDims {
        self.button.get_dims()
    }

    fn set_pos(&mut self, top_left: ScreenPt, total_width: f64) {
        self.button.set_pos(top_left, total_width);
        // TODO Brittle, convenient only for where these buttons are being placed right now
        self.menu.set_pos(
            ScreenPt::new(top_left.x, top_left.y + self.get_dims().height),
            total_width,
        );
    }
}
