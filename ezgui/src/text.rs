// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use crate::{Canvas, Color, GfxCtx, ScreenPt};
use graphics;
use graphics::character::CharacterCache;
use graphics::{Rectangle, Transformed};
use opengl_graphics::GlyphCache;
use textwrap;

pub const TEXT_FG_COLOR: Color = Color([0.0, 0.0, 0.0, 1.0]);
pub const TEXT_QUERY_COLOR: Color = Color([0.0, 0.0, 1.0, 0.5]);
pub const TEXT_FOCUS_COLOR: Color = Color([1.0, 0.0, 0.0, 0.5]);
const TEXT_BG_COLOR: Color = Color([0.0, 1.0, 0.0, 0.5]);

const FONT_SIZE: u32 = 24;
// TODO These are dependent on FONT_SIZE, but hand-tuned. Glyphs all have 0 as their height, and
// they need adjustments to their positioning.
pub const LINE_HEIGHT: f64 = 32.0;
const SHIFT_TEXT_UP: f64 = 7.0;
const MAX_CHAR_WIDTH: f64 = 25.0;

#[derive(Debug, Clone)]
struct TextSpan {
    text: String,
    fg_color: Color,
    // The Text's bg_color will cover the entire block, but some parts can have extra highlighting.
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

// TODO parse style from markup tags
#[derive(Debug, Clone)]
pub struct Text {
    lines: Vec<Vec<TextSpan>>,
    bg_color: Option<Color>,
}

impl Text {
    pub fn new() -> Text {
        Text {
            lines: Vec::new(),
            bg_color: Some(TEXT_BG_COLOR),
        }
    }

    pub fn with_bg_color(bg_color: Option<Color>) -> Text {
        Text {
            lines: Vec::new(),
            bg_color,
        }
    }

    pub fn from_line(line: String) -> Text {
        let mut txt = Text::new();
        txt.add_line(line);
        txt
    }

    pub fn pad_if_nonempty(&mut self) {
        if !self.lines.is_empty() {
            self.lines.push(Vec::new());
        }
    }

    pub fn add_line(&mut self, line: String) {
        self.lines.push(vec![TextSpan::default_style(line)]);
    }

    // TODO Ideally we'd wrap last-minute when drawing, but eh, start somewhere.
    pub fn add_wrapped_line(&mut self, canvas: &Canvas, line: String) {
        let wrap_to = canvas.window_width / MAX_CHAR_WIDTH;
        for l in textwrap::wrap(&line, wrap_to as usize).into_iter() {
            self.add_line(l.to_string());
        }
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
        if self.lines.is_empty() {
            self.lines.push(Vec::new());
        }

        self.lines.last_mut().unwrap().push(TextSpan {
            text,
            fg_color,
            highlight_color,
        });
    }

    pub fn num_lines(&self) -> usize {
        self.lines.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub(crate) fn dims(&self, glyphs: &mut GlyphCache) -> (f64, f64) {
        let longest_line = self
            .lines
            .iter()
            .max_by_key(|l| l.iter().fold(0, |so_far, span| so_far + span.text.len()))
            .unwrap();
        let mut concat = String::new();
        for span in longest_line {
            concat.push_str(&span.text);
        }
        let width = glyphs.width(FONT_SIZE, &concat).unwrap();
        let height = (self.lines.len() as f64) * LINE_HEIGHT;
        (width, height)
    }
}

pub fn draw_text_bubble(g: &mut GfxCtx, glyphs: &mut GlyphCache, top_left: ScreenPt, txt: Text) {
    let (total_width, total_height) = txt.dims(glyphs);
    if let Some(c) = txt.bg_color {
        Rectangle::new(c.0).draw(
            [top_left.x, top_left.y, total_width, total_height],
            &g.orig_ctx.draw_state,
            g.orig_ctx.transform,
            g.gfx,
        );
    }

    let mut y = top_left.y + LINE_HEIGHT;
    for line in &txt.lines {
        let mut x = top_left.x;

        let first_bg_color = line[0].highlight_color;
        let mut same_bg_color = true;
        for (idx, span) in line.into_iter().enumerate() {
            if span.highlight_color != first_bg_color {
                same_bg_color = false;
            }

            let span_width = glyphs.width(FONT_SIZE, &span.text).unwrap();
            if let Some(color) = span.highlight_color {
                // If this is the last span and all spans use the same background color, then
                // extend the background over the entire width of the text box.
                let width = if idx == line.len() - 1 && same_bg_color {
                    total_width - (x - top_left.x)
                } else {
                    span_width
                };
                Rectangle::new(color.0).draw(
                    [x, y - LINE_HEIGHT, width, LINE_HEIGHT],
                    &g.orig_ctx.draw_state,
                    g.orig_ctx.transform,
                    g.gfx,
                );
            }

            graphics::Text::new_color(span.fg_color.0, FONT_SIZE)
                .draw(
                    &span.text,
                    glyphs,
                    &g.orig_ctx.draw_state,
                    g.orig_ctx.transform.trans(x, y - SHIFT_TEXT_UP),
                    g.gfx,
                )
                .unwrap();
            x += span_width;
        }
        y += LINE_HEIGHT;
    }
}
