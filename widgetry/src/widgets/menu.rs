use geom::Pt2D;

use crate::{
    Choice, EventCtx, GfxCtx, Key, Line, Outcome, ScreenDims, ScreenPt, ScreenRectangle, Style,
    Text, Widget, WidgetImpl, WidgetOutput,
};

pub struct Menu<T> {
    choices: Vec<Choice<T>>,
    current_idx: usize,

    pub(crate) top_left: ScreenPt,
    dims: ScreenDims,
}

impl<T: 'static> Menu<T> {
    pub fn widget(ctx: &EventCtx, choices: Vec<Choice<T>>) -> Widget {
        Widget::new(Box::new(Self::new(ctx, choices)))
    }

    pub fn new(ctx: &EventCtx, choices: Vec<Choice<T>>) -> Self {
        let mut m = Menu {
            choices,
            current_idx: 0,

            top_left: ScreenPt::new(0.0, 0.0),
            dims: ScreenDims::new(0.0, 0.0),
        };
        m.dims = m.calculate_txt(ctx.style()).dims(&ctx.prerender.assets);
        m
    }

    pub fn take_current_choice(&mut self) -> T {
        // TODO Make sure it's marked invalid, like button
        self.choices.remove(self.current_idx).data
    }

    pub fn set_current(&mut self, idx: usize) {
        self.current_idx = idx;
    }

    fn calculate_txt(&self, style: &Style) -> Text {
        let mut txt = Text::new();

        for (idx, choice) in self.choices.iter().enumerate() {
            let is_hovered = idx == self.current_idx;
            let mut text_color = if is_hovered {
                choice.fg.unwrap_or(style.btn_solid.fg)
            } else {
                choice.fg.unwrap_or(style.text_fg_color)
            };

            if choice.active {
                if let Some(ref key) = choice.hotkey {
                    txt.add_appended(vec![
                        Line(key.describe()).fg(style.text_hotkey_color),
                        Line(format!(" - {}", choice.label)).fg(text_color),
                    ]);
                } else {
                    txt.add_line(Line(&choice.label).fg(text_color))
                }
            } else {
                text_color = text_color.alpha(0.8);
                if let Some(ref key) = choice.hotkey {
                    txt.add_line(
                        Line(format!("{} - {}", key.describe(), choice.label)).fg(text_color),
                    );
                } else {
                    txt.add_line(Line(&choice.label).fg(text_color));
                }
            }

            if choice.tooltip.is_some() {
                // TODO Ideally unicode info symbol, but the fonts don't seem to have it
                txt.append(Line(" (!)"));
            }

            // TODO BG color should be on the TextSpan, so this isn't so terrible?
            if is_hovered {
                txt.highlight_last_line(style.btn_solid.bg);
            }
        }
        txt
    }
}

impl<T: 'static> WidgetImpl for Menu<T> {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.top_left = top_left;
    }

    fn event(&mut self, ctx: &mut EventCtx, output: &mut WidgetOutput) {
        if self.choices.is_empty() {
            return;
        }

        // Handle the mouse
        if ctx.redo_mouseover() {
            if let Some(cursor) = ctx.canvas.get_cursor_in_screen_space() {
                let mut top_left = self.top_left;
                for idx in 0..self.choices.len() {
                    let rect = ScreenRectangle {
                        x1: top_left.x,
                        y1: top_left.y,
                        x2: top_left.x + self.dims.width,
                        y2: top_left.y + ctx.default_line_height(),
                    };
                    if rect.contains(cursor) && self.choices[idx].active {
                        self.current_idx = idx;
                        break;
                    }
                    top_left.y += ctx.default_line_height();
                }
            }
        }
        {
            let choice = &self.choices[self.current_idx];
            if ctx.normal_left_click() {
                // Did we actually click the entry?
                let mut top_left = self.top_left;
                top_left.y += ctx.default_line_height() * (self.current_idx as f64);
                let rect = ScreenRectangle {
                    x1: top_left.x,
                    y1: top_left.y,
                    x2: top_left.x + self.dims.width,
                    y2: top_left.y + ctx.default_line_height(),
                };
                if let Some(pt) = ctx.canvas.get_cursor_in_screen_space() {
                    if rect.contains(pt) && choice.active {
                        output.outcome = Outcome::Clicked(choice.label.clone());
                        return;
                    }
                }
                ctx.input.unconsume_event();
            }
        }

        // Handle hotkeys
        for (idx, choice) in self.choices.iter().enumerate() {
            if !choice.active {
                continue;
            }
            if ctx.input.pressed(choice.hotkey.clone()) {
                self.current_idx = idx;
                output.outcome = Outcome::Clicked(choice.label.clone());
                return;
            }
        }

        // Handle nav keys
        #[allow(clippy::collapsible_if)]
        if ctx.input.pressed(Key::Enter) {
            let choice = &self.choices[self.current_idx];
            if choice.active {
                output.outcome = Outcome::Clicked(choice.label.clone());
            }
        } else if ctx.input.pressed(Key::UpArrow) {
            if self.current_idx > 0 {
                self.current_idx -= 1;
            }
        } else if ctx.input.pressed(Key::DownArrow) {
            if self.current_idx < self.choices.len() - 1 {
                self.current_idx += 1;
            }
        }
    }

    fn draw(&self, g: &mut GfxCtx) {
        if self.choices.is_empty() {
            return;
        }

        let draw = g.upload(self.calculate_txt(g.style()).render(g));
        g.fork(
            Pt2D::new(0.0, 0.0),
            self.top_left,
            1.0,
            Some(crate::drawing::MENU_Z),
        );
        g.redraw(&draw);
        g.unfork();

        if let Some(ref info) = self.choices[self.current_idx].tooltip {
            // Hold on, are we actually hovering on that entry right now?
            let mut top_left = self.top_left;
            top_left.y += g.default_line_height() * (self.current_idx as f64);
            let rect = ScreenRectangle {
                x1: top_left.x,
                y1: top_left.y,
                x2: top_left.x + self.dims.width,
                y2: top_left.y + g.default_line_height(),
            };
            if let Some(pt) = g.canvas.get_cursor_in_screen_space() {
                if rect.contains(pt) {
                    g.draw_mouse_tooltip(
                        Text::from(info)
                            .inner_wrap_to_pct(0.3 * g.canvas.window_width, &g.prerender.assets),
                    );
                }
            }
        }
    }
}
