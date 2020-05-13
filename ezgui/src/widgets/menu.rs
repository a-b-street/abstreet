use crate::{
    hotkey, text, Choice, EventCtx, GfxCtx, InputResult, Key, Line, ScreenDims, ScreenPt,
    ScreenRectangle, Text, Widget, WidgetImpl, WidgetOutput,
};
use geom::Pt2D;

pub struct Menu<T: Clone> {
    choices: Vec<Choice<T>>,
    current_idx: usize,

    pub(crate) state: InputResult<T>,

    pub(crate) top_left: ScreenPt,
    dims: ScreenDims,
}

impl<T: 'static + Clone> Menu<T> {
    pub fn new(ctx: &EventCtx, choices: Vec<Choice<T>>) -> Widget {
        let mut m = Menu {
            choices,
            current_idx: 0,

            state: InputResult::StillActive,

            top_left: ScreenPt::new(0.0, 0.0),
            dims: ScreenDims::new(0.0, 0.0),
        };
        m.dims = m.calculate_txt().dims(&ctx.prerender.assets);
        Widget::new(Box::new(m))
    }

    pub fn current_choice(&self) -> &T {
        &self.choices[self.current_idx].data
    }

    fn calculate_txt(&self) -> Text {
        let mut txt = Text::new();

        for (idx, choice) in self.choices.iter().enumerate() {
            if choice.active {
                if let Some(ref key) = choice.hotkey {
                    txt.add_appended(vec![
                        Line(key.describe()),
                        Line(format!(" - {}", choice.label)),
                    ]);
                } else {
                    txt.add(Line(&choice.label));
                }
            } else {
                if let Some(ref key) = choice.hotkey {
                    txt.add(
                        Line(format!("{} - {}", key.describe(), choice.label))
                            .fg(text::INACTIVE_CHOICE_COLOR),
                    );
                } else {
                    txt.add(Line(&choice.label).fg(text::INACTIVE_CHOICE_COLOR));
                }
            }
            if choice.tooltip.is_some() {
                // TODO Ideally unicode info symbol, but the fonts don't seem to have it
                txt.append(Line(" (!)"));
            }

            // TODO BG color should be on the TextSpan, so this isn't so terrible?
            if idx == self.current_idx {
                txt.highlight_last_line(text::SELECTED_COLOR);
            }
        }
        txt
    }
}

impl<T: 'static + Clone> WidgetImpl for Menu<T> {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.top_left = top_left;
    }

    fn event(&mut self, ctx: &mut EventCtx, _output: &mut WidgetOutput) {
        if self.choices.is_empty() {
            return;
        }

        match self.state {
            InputResult::StillActive => {}
            _ => unreachable!(),
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
                    if rect.contains(cursor) {
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
                top_left.y += ctx.default_line_height() * (self.current_idx as f32);
                let rect = ScreenRectangle {
                    x1: top_left.x,
                    y1: top_left.y,
                    x2: top_left.x + self.dims.width,
                    y2: top_left.y + ctx.default_line_height(),
                };
                if let Some(pt) = ctx.canvas.get_cursor_in_screen_space() {
                    if rect.contains(pt) && choice.active {
                        self.state = InputResult::Done(choice.label.clone(), choice.data.clone());
                        return;
                    }
                    // Unconsume the click, it was in screen space, but not on us.
                    ctx.input.unconsume_event();
                } else {
                    // Clicked on the map? Cancel out
                    self.state = InputResult::Canceled;
                    return;
                }
            }
        }

        // Handle hotkeys
        for choice in &self.choices {
            if !choice.active {
                continue;
            }
            if let Some(ref hotkey) = choice.hotkey {
                if ctx.input.new_was_pressed(hotkey) {
                    self.state = InputResult::Done(choice.label.clone(), choice.data.clone());
                    return;
                }
            }
        }

        // Handle nav keys
        if ctx.input.new_was_pressed(&hotkey(Key::Enter).unwrap()) {
            let choice = &self.choices[self.current_idx];
            if choice.active {
                self.state = InputResult::Done(choice.label.clone(), choice.data.clone());
                return;
            } else {
                return;
            }
        } else if ctx.input.new_was_pressed(&hotkey(Key::UpArrow).unwrap()) {
            if self.current_idx > 0 {
                self.current_idx -= 1;
            }
        } else if ctx.input.new_was_pressed(&hotkey(Key::DownArrow).unwrap()) {
            if self.current_idx < self.choices.len() - 1 {
                self.current_idx += 1;
            }
        }
    }

    fn draw(&self, g: &mut GfxCtx) {
        if self.choices.is_empty() {
            return;
        }

        let draw = g.upload(self.calculate_txt().render_g(g));
        // In between tooltip and normal screenspace
        g.fork(Pt2D::new(0.0, 0.0), self.top_left, 1.0, Some(0.1));
        g.redraw(&draw);
        g.unfork();

        if let Some(ref info) = self.choices[self.current_idx].tooltip {
            // Hold on, are we actually hovering on that entry right now?
            let mut top_left = self.top_left;
            top_left.y += g.default_line_height() * (self.current_idx as f32);
            let rect = ScreenRectangle {
                x1: top_left.x,
                y1: top_left.y,
                x2: top_left.x + self.dims.width,
                y2: top_left.y + g.default_line_height(),
            };
            if let Some(pt) = g.canvas.get_cursor_in_screen_space() {
                if rect.contains(pt) {
                    let mut txt = Text::new();
                    txt.add_wrapped(info.to_string(), 0.5 * g.canvas.window_width);
                    g.draw_mouse_tooltip(txt);
                }
            }
        }
    }
}
