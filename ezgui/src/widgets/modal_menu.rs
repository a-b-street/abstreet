use crate::layout::Widget;
use crate::widgets::Button;
use crate::{
    layout, text, EventCtx, GfxCtx, Line, MultiKey, ScreenDims, ScreenPt, ScreenRectangle, Text,
};

// TODO No separators

pub struct ModalMenu {
    title: String,
    info: Text,
    chosen_action: Option<String>,
    choices: Vec<Choice>,
    // This can be inactive entries too.
    hovering_idx: Option<usize>,

    show_hide_btn: Button,
    // TODO Actually, 3 states: full, no controls, just title
    visible: bool,

    top_left: ScreenPt,
    dims: ScreenDims,
}

struct Choice {
    hotkey: Option<MultiKey>,
    label: String,
    active: bool,
}

impl ModalMenu {
    pub fn new<S: Into<String>>(
        title: S,
        raw_choice_groups: Vec<Vec<(Option<MultiKey>, &str)>>,
        ctx: &EventCtx,
    ) -> ModalMenu {
        let mut choices = Vec::new();
        for group in raw_choice_groups {
            for (hotkey, label) in group {
                choices.push(Choice {
                    hotkey,
                    label: label.to_string(),
                    active: false,
                });
            }
        }

        let mut m = ModalMenu {
            title: title.into(),
            info: Text::new(),
            chosen_action: None,
            choices,
            hovering_idx: None,

            show_hide_btn: Button::hide_btn(ctx),
            visible: true,

            top_left: ScreenPt::new(0.0, 0.0),
            dims: ScreenDims::new(0.0, 0.0),
        };
        m.recalculate_dims(ctx);

        // TODO For legacy behavior, standalone menus
        layout::stack_vertically(
            layout::ContainerOrientation::TopRight,
            ctx.canvas,
            vec![&mut m],
        );

        m
    }

    pub fn set_info(&mut self, ctx: &EventCtx, info: Text) {
        self.info = info;
        self.recalculate_dims(ctx);
    }

    pub fn event(&mut self, ctx: &mut EventCtx) {
        if let Some(ref action) = self.chosen_action {
            panic!("Caller didn't consume modal action '{}'", action);
        }

        // Handle the mouse
        if self.visible && ctx.redo_mouseover() {
            let cursor = ctx.canvas.get_cursor_in_screen_space();
            self.hovering_idx = None;
            let mut top_left = self.top_left;
            top_left.y += ctx.canvas.line_height + ctx.canvas.text_dims(&self.info).1;
            for idx in 0..self.choices.len() {
                let rect = ScreenRectangle {
                    x1: top_left.x,
                    y1: top_left.y,
                    x2: top_left.x + self.dims.width,
                    y2: top_left.y + ctx.canvas.line_height,
                };
                if rect.contains(cursor) {
                    self.hovering_idx = Some(idx);
                    break;
                }
                top_left.y += ctx.canvas.line_height;
            }
        }
        if let Some(idx) = self.hovering_idx {
            if ctx.input.left_mouse_button_pressed() && self.choices[idx].active {
                self.chosen_action = Some(self.choices[idx].label.clone());
            }
        }

        // Handle hotkeys
        for choice in &self.choices {
            if !choice.active {
                continue;
            }
            if let Some(hotkey) = choice.hotkey {
                if ctx.input.new_was_pressed(hotkey) {
                    self.chosen_action = Some(choice.label.clone());
                    break;
                }
            }
        }

        // Handle showing/hiding
        // TODO Layouting of nested widgets...
        self.show_hide_btn.set_pos(
            ScreenPt::new(self.top_left.x + self.dims.width, self.top_left.y),
            self.dims.width,
        );
        self.show_hide_btn.event(ctx);
        if self.show_hide_btn.clicked() {
            self.visible = !self.visible;
            if self.visible {
                self.show_hide_btn = Button::hide_btn(ctx);
            } else {
                self.show_hide_btn = Button::show_btn(ctx);
                self.hovering_idx = None;
            }
            self.show_hide_btn.just_replaced();
            self.recalculate_dims(ctx);
        }

        // Reset for next round
        for choice in self.choices.iter_mut() {
            choice.active = false;
        }
    }

    pub fn push_action(&mut self, hotkey: Option<MultiKey>, label: &str, ctx: &EventCtx) {
        self.choices.push(Choice {
            hotkey,
            label: label.to_string(),
            active: false,
        });
        self.recalculate_dims(ctx);
    }

    pub fn remove_action(&mut self, label: &str, ctx: &EventCtx) {
        self.choices.retain(|c| c.label != label);
        self.recalculate_dims(ctx);
    }

    pub fn change_action(&mut self, old_label: &str, new_label: &str, ctx: &EventCtx) {
        for c in self.choices.iter_mut() {
            if c.label == old_label {
                c.label = new_label.to_string();
                self.recalculate_dims(ctx);
                return;
            }
        }
        panic!("Menu doesn't have {}", old_label);
    }

    pub fn swap_action(&mut self, old_label: &str, new_label: &str, ctx: &EventCtx) -> bool {
        if self.action(old_label) {
            self.change_action(old_label, new_label, ctx);
            true
        } else {
            false
        }
    }

    pub fn consume_action(&mut self, name: &str, ctx: &EventCtx) -> bool {
        if self.action(name) {
            self.remove_action(name, ctx);
            true
        } else {
            false
        }
    }

    pub fn action(&mut self, label: &str) -> bool {
        if let Some(ref action) = self.chosen_action {
            if label == action {
                self.chosen_action = None;
                return true;
            }
            return false;
        }

        for c in self.choices.iter_mut() {
            if c.label == label {
                c.active = true;
                return false;
            }
        }
        panic!("Menu doesn't have action {}", label);
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        g.draw_text_at_screenspace_topleft(&self.calculate_txt(), self.top_left);
        self.show_hide_btn.draw(g);
    }

    fn recalculate_dims(&mut self, ctx: &EventCtx) {
        let (w, h) = ctx.canvas.text_dims(&self.calculate_txt());
        self.dims = ScreenDims::new(w, h);
    }

    fn calculate_txt(&self) -> Text {
        let mut txt = Text::prompt(&self.title);
        if !self.visible {
            return txt;
        }
        txt.extend(&self.info);

        for (idx, choice) in self.choices.iter().enumerate() {
            if choice.active {
                if let Some(key) = choice.hotkey {
                    txt.add_appended(vec![
                        Line(key.describe()).fg(text::HOTKEY_COLOR),
                        Line(format!(" - {}", choice.label)),
                    ]);
                } else {
                    txt.add(Line(&choice.label));
                }

                // TODO BG color should be on the TextSpan, so this isn't so terrible?
                if Some(idx) == self.hovering_idx {
                    txt.highlight_last_line(text::SELECTED_COLOR);
                }
            } else {
                if let Some(key) = choice.hotkey {
                    txt.add(
                        Line(format!("{} - {}", key.describe(), choice.label))
                            .fg(text::INACTIVE_CHOICE_COLOR),
                    );
                } else {
                    txt.add(Line(&choice.label).fg(text::INACTIVE_CHOICE_COLOR));
                }
            }
        }
        txt
    }
}

impl Widget for ModalMenu {
    fn get_dims(&self) -> ScreenDims {
        ScreenDims::new(
            self.dims.width + self.show_hide_btn.get_dims().width,
            self.dims.height,
        )
    }

    fn set_pos(&mut self, top_left: ScreenPt, _total_width: f64) {
        self.top_left = top_left;
        // TODO Stretch to fill total width if it's smaller than us? Or that's impossible
    }
}
