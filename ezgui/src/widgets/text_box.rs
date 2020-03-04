use crate::layout::Widget;
use crate::{text, EventCtx, GfxCtx, Key, Line, ScreenDims, ScreenPt, Text};

// TODO right now, only a single line

pub struct TextBox {
    // TODO A rope would be cool.
    line: String,
    cursor_x: usize,
    shift_pressed: bool,

    top_left: ScreenPt,
    dims: ScreenDims,
}

impl TextBox {
    pub fn new(ctx: &EventCtx, max_chars: usize, prefilled: String) -> TextBox {
        TextBox {
            cursor_x: prefilled.len(),
            line: prefilled,
            shift_pressed: false,

            top_left: ScreenPt::new(0.0, 0.0),
            dims: ScreenDims::new(
                (max_chars as f64) * text::MAX_CHAR_WIDTH,
                ctx.default_line_height(),
            ),
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx) {
        if let Some(key) = ctx.input.any_key_pressed() {
            match key {
                Key::LeftShift => {
                    self.shift_pressed = true;
                }
                Key::LeftArrow => {
                    if self.cursor_x > 0 {
                        self.cursor_x -= 1;
                    }
                }
                Key::RightArrow => {
                    self.cursor_x = (self.cursor_x + 1).min(self.line.len());
                }
                Key::Backspace => {
                    if self.cursor_x > 0 {
                        self.line.remove(self.cursor_x - 1);
                        self.cursor_x -= 1;
                    }
                }
                _ => {
                    if let Some(c) = key.to_char(self.shift_pressed) {
                        self.line.insert(self.cursor_x, c);
                        self.cursor_x += 1;
                    } else {
                        ctx.input.unconsume_event();
                    }
                }
            };
        }
        if ctx.input.key_released(Key::LeftShift) {
            self.shift_pressed = false;
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        g.draw_blocking_text_at_screenspace_topleft(self.calculate_text(), self.top_left);
    }

    pub fn get_entry(&self) -> String {
        self.line.clone()
    }

    fn calculate_text(&self) -> Text {
        let mut txt = Text::from(Line(&self.line[0..self.cursor_x]));
        if self.cursor_x < self.line.len() {
            // TODO This "cursor" looks awful!
            txt.append_all(vec![
                Line("|").fg(text::SELECTED_COLOR),
                Line(&self.line[self.cursor_x..=self.cursor_x]),
                Line(&self.line[self.cursor_x + 1..]),
            ]);
        } else {
            txt.append(Line("|").fg(text::SELECTED_COLOR));
        }
        txt
    }
}

impl Widget for TextBox {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.top_left = top_left;
    }
}
