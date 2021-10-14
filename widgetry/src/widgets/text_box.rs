use geom::{Distance, Polygon};

use crate::{
    EdgeInsets, EventCtx, GeomBatch, GfxCtx, Key, Line, Outcome, ScreenDims, ScreenPt,
    ScreenRectangle, Style, Text, Widget, WidgetImpl, WidgetOutput,
};

// TODO right now, only a single line
// TODO max_chars isn't enforced; you can type as much as you want...

pub struct TextBox {
    line: String,
    label: String,
    cursor_x: usize,
    has_focus: bool,
    hovering: bool,
    autofocus: bool,
    padding: EdgeInsets,

    top_left: ScreenPt,
    dims: ScreenDims,
}

impl TextBox {
    // TODO Really should have an options struct with defaults
    pub fn default_widget<I: Into<String>>(ctx: &EventCtx, label: I, prefilled: String) -> Widget {
        TextBox::widget(ctx, label, prefilled, true, 50)
    }

    pub fn widget<I: Into<String>>(
        ctx: &EventCtx,
        label: I,
        prefilled: String,
        autofocus: bool,
        max_chars: usize,
    ) -> Widget {
        let label = label.into();
        Widget::new(Box::new(TextBox::new(
            ctx,
            label.clone(),
            max_chars,
            prefilled,
            autofocus,
        )))
        .named(label)
    }

    pub(crate) fn new(
        ctx: &EventCtx,
        label: String,
        max_chars: usize,
        prefilled: String,
        autofocus: bool,
    ) -> TextBox {
        let padding = EdgeInsets {
            top: 6.0,
            left: 8.0,
            bottom: 8.0,
            right: 8.0,
        };
        let max_char_width = 25.0;
        TextBox {
            label,
            cursor_x: prefilled.len(),
            line: prefilled,
            has_focus: false,
            hovering: false,
            autofocus,
            padding,
            top_left: ScreenPt::new(0.0, 0.0),
            dims: ScreenDims::new(
                (max_chars as f64) * max_char_width + (padding.left + padding.right) as f64,
                ctx.default_line_height() + (padding.top + padding.bottom) as f64,
            ),
        }
    }

    fn calculate_text(&self, style: &Style) -> Text {
        let mut txt = Text::from(&self.line[0..self.cursor_x]);
        if self.cursor_x < self.line.len() {
            // TODO This "cursor" looks awful!
            txt.append_all(vec![
                Line("|").fg(style.text_primary_color),
                Line(&self.line[self.cursor_x..=self.cursor_x]),
                Line(&self.line[self.cursor_x + 1..]),
            ]);
        } else {
            txt.append(Line("|").fg(style.text_primary_color));
        }
        txt
    }

    pub fn get_line(&self) -> String {
        self.line.clone()
    }
}

impl WidgetImpl for TextBox {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.top_left = top_left;
    }

    fn event(&mut self, ctx: &mut EventCtx, output: &mut WidgetOutput) {
        if ctx.redo_mouseover() {
            if let Some(pt) = ctx.canvas.get_cursor_in_screen_space() {
                self.hovering = ScreenRectangle::top_left(self.top_left, self.dims).contains(pt);
            } else {
                self.hovering = false;
            }
        }

        // TODO Revisit this, which was originally written for a panel containing multiple
        // textboxes.
        if ctx.normal_left_click() {
            // Let all textboxes see this event, so they can deactivate their own focus.
            // TODO But if a button is clicked before this textbox, that event isn't seen here...
            ctx.input.unconsume_event();
            self.has_focus = self.hovering;
        }

        if !self.has_focus && !self.autofocus {
            return;
        }
        if let Some(key) = ctx.input.any_pressed() {
            match key {
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
                        output.outcome = Outcome::Changed(self.label.clone());
                        self.line.remove(self.cursor_x - 1);
                        self.cursor_x -= 1;
                    }
                }
                _ => {
                    if let Some(c) = key.to_char(ctx.is_key_down(Key::LeftShift)) {
                        output.outcome = Outcome::Changed(self.label.clone());
                        self.line.insert(self.cursor_x, c);
                        self.cursor_x += 1;
                    } else {
                        ctx.input.unconsume_event();
                    }
                }
            };
        }
    }

    fn draw(&self, g: &mut GfxCtx) {
        // TODO Cache
        let mut batch = GeomBatch::from(vec![(
            g.style().field_bg,
            Polygon::rounded_rectangle(self.dims.width, self.dims.height, 2.0),
        )]);

        let outline_style = g.style().btn_outline.outline;
        if let Ok(outline) = Polygon::rounded_rectangle(self.dims.width, self.dims.height, 2.0)
            .to_outline(Distance::meters(outline_style.0))
        {
            batch.push(outline_style.1, outline);
        }

        batch.append(
            self.calculate_text(g.style())
                .render_autocropped(g)
                .translate(self.padding.left, self.padding.top),
        );
        let draw = g.upload(batch);
        g.redraw_at(self.top_left, &draw);
    }
}
