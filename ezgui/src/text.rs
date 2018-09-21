// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use graphics;
use graphics::types::Color;
use graphics::{Image, Transformed};
use GfxCtx;

pub const TEXT_FG_COLOR: Color = [0.0, 0.0, 0.0, 1.0];
pub const TEXT_QUERY_COLOR: Color = [0.0, 0.0, 1.0, 0.5];
pub const TEXT_FOCUS_COLOR: Color = [1.0, 0.0, 0.0, 0.5];
const TEXT_BG_COLOR: Color = [0.0, 1.0, 0.0, 0.5];

const FONT_SIZE: u32 = 24;
// TODO this is a hack, need a glyphs.height() method as well!
const LINE_HEIGHT: f64 = 22.0;

struct TextSpan {
    text: String,
    fg_color: Color,
    // The TextOSD's bg_color will cover the entire block, but some parts can have extra
    // highlighting.
    highlight_color: Option<Color>,
    // TODO bold, italic, font size, font style
}

impl TextSpan {
    fn default_style(text: String) -> TextSpan {
        TextSpan {
            text,
            fg_color: TEXT_FG_COLOR,
            highlight_color: None,
        }
    }
}

// TODO rename "RichText" or something?
// TODO parse style from markup tags
pub struct TextOSD {
    lines: Vec<Vec<TextSpan>>,
    bg_color: Color,
}

impl TextOSD {
    pub fn new() -> TextOSD {
        TextOSD {
            lines: Vec::new(),
            bg_color: TEXT_BG_COLOR,
        }
    }

    pub fn pad_if_nonempty(&mut self) {
        if !self.lines.is_empty() {
            self.lines.push(Vec::new());
        }
    }

    pub fn add_line(&mut self, line: String) {
        self.lines.push(vec![TextSpan::default_style(line)]);
    }

    pub fn add_styled_line(
        &mut self,
        line: String,
        fg_color: Color,
        highlight_color: Option<Color>,
    ) {
        self.lines.push(vec![TextSpan {
            text: line,
            fg_color,
            highlight_color,
        }]);
    }

    pub fn append(&mut self, text: String, fg_color: Color, highlight_color: Option<Color>) {
        self.lines.last_mut().unwrap().push(TextSpan {
            text,
            fg_color,
            highlight_color,
        });
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn dims(&self, g: &mut GfxCtx) -> (f64, f64) {
        let longest_line = self
            .lines
            .iter()
            .max_by_key(|l| l.iter().fold(0, |so_far, span| so_far + span.text.len()))
            .unwrap();
        let mut concat = String::from("");
        for span in longest_line {
            concat.push_str(&span.text);
        }
        let width = g.glyphs.width(FONT_SIZE, &concat).unwrap();
        let height = (self.lines.len() as f64) * LINE_HEIGHT;
        (width, height)
    }
}

pub fn draw_text_bubble(g: &mut GfxCtx, (x1, y1): (f64, f64), osd: TextOSD) {
    let (total_width, total_height) = osd.dims(g);
    graphics::Rectangle::new(osd.bg_color).draw(
        [x1, y1, total_width, total_height],
        &g.orig_ctx.draw_state,
        g.orig_ctx.transform,
        g.gfx,
    );

    let mut y = y1 + LINE_HEIGHT;
    for line in &osd.lines {
        let mut x = x1;

        for span in line {
            if let Some(color) = span.highlight_color {
                // TODO do we ever want to use total_width?
                let width = g.glyphs.width(FONT_SIZE, &span.text).unwrap();
                graphics::Rectangle::new(color).draw(
                    [x, y - LINE_HEIGHT, width, LINE_HEIGHT],
                    &g.orig_ctx.draw_state,
                    g.orig_ctx.transform,
                    g.gfx,
                );
            }

            let fg_text = Image::new_color(span.fg_color);

            for ch in span.text.chars() {
                if let Ok(draw_ch) = g.glyphs.character(FONT_SIZE, ch) {
                    let char_ctx = g
                        .orig_ctx
                        .transform
                        .trans(x + draw_ch.left(), y - draw_ch.top());
                    fg_text.draw(draw_ch.texture, &g.orig_ctx.draw_state, char_ctx, g.gfx);
                    x += draw_ch.width();
                } else {
                    panic!("Couldn't get glyph for {}", ch);
                }
            }
        }
        y += LINE_HEIGHT;
    }
}
