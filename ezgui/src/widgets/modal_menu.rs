use crate::layout::Widget;
use crate::{
    layout, text, EventCtx, GfxCtx, Line, MultiKey, ScreenDims, ScreenPt, ScreenRectangle, Text,
};

pub struct ModalMenu {
    title: String,
    info: Text,
    chosen_action: Option<String>,
    choices: Vec<Choice>,
    // This can be inactive entries too.
    hovering_idx: Option<usize>,
    standalone_layout: Option<layout::ContainerOrientation>,

    top_left: ScreenPt,
    dims: ScreenDims,
}

struct Choice {
    hotkey: Option<MultiKey>,
    label: String,
    active: bool,
}

impl ModalMenu {
    pub fn new<S1: Into<String>, S2: Into<String>>(
        title: S1,
        raw_choices: Vec<(Option<MultiKey>, S2)>,
        ctx: &EventCtx,
    ) -> ModalMenu {
        let mut m = ModalMenu {
            title: title.into(),
            info: Text::new().with_bg(),
            chosen_action: None,
            choices: raw_choices
                .into_iter()
                .map(|(hotkey, label)| Choice {
                    hotkey,
                    label: label.into(),
                    active: false,
                })
                .collect(),
            hovering_idx: None,
            standalone_layout: Some(layout::ContainerOrientation::TopRight),

            top_left: ScreenPt::new(0.0, 0.0),
            dims: ScreenDims::new(0.0, 0.0),
        };
        m.recalculate_dims(ctx);

        m
    }

    // It's part of something bigger
    pub fn disable_standalone_layout(mut self) -> ModalMenu {
        assert!(self.standalone_layout.is_some());
        self.standalone_layout = None;
        self
    }

    pub fn set_standalone_layout(mut self, layout: layout::ContainerOrientation) -> ModalMenu {
        self.standalone_layout = Some(layout);
        self
    }

    pub fn set_info(&mut self, ctx: &EventCtx, info: Text) {
        self.info = info.with_bg();
        self.recalculate_dims(ctx);
    }

    pub fn event(&mut self, ctx: &mut EventCtx) {
        if let Some(ref action) = self.chosen_action {
            panic!("Caller didn't consume modal action '{}'", action);
        }

        if let Some(o) = self.standalone_layout {
            layout::stack_vertically(o, ctx, vec![self]);
            self.recalculate_dims(ctx);
        }

        // Handle the mouse
        if ctx.redo_mouseover() {
            self.hovering_idx = None;
            if let Some(cursor) = ctx.canvas.get_cursor_in_screen_space() {
                let mut top_left = self.top_left;
                top_left.y += ctx.text_dims(&self.info).height;
                if !self.title.is_empty() {
                    top_left.y += ctx.default_line_height();
                }
                for idx in 0..self.choices.len() {
                    let rect = ScreenRectangle {
                        x1: top_left.x,
                        y1: top_left.y,
                        x2: top_left.x + self.dims.width,
                        y2: top_left.y + ctx.default_line_height(),
                    };
                    if rect.contains(cursor) {
                        self.hovering_idx = Some(idx);
                        break;
                    }
                    top_left.y += ctx.default_line_height();
                }
            }
        }
        if let Some(idx) = self.hovering_idx {
            if ctx.normal_left_click() && self.choices[idx].active {
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

    pub fn maybe_change_action(&mut self, old_label: &str, new_label: &str, ctx: &EventCtx) {
        for c in self.choices.iter_mut() {
            if c.label == old_label {
                c.label = new_label.to_string();
                self.recalculate_dims(ctx);
                return;
            }
        }
        // Don't panic
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
        g.draw_blocking_text_at_screenspace_topleft(&self.calculate_txt(), self.top_left);
    }

    fn recalculate_dims(&mut self, ctx: &EventCtx) {
        self.dims = ctx.text_dims(&self.calculate_txt());
    }

    fn calculate_txt(&self) -> Text {
        let mut txt = if self.title.is_empty() {
            Text::new().with_bg()
        } else {
            Text::prompt(&self.title)
        };
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
        ScreenDims::new(self.dims.width, self.dims.height)
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.top_left = top_left;
    }
}
