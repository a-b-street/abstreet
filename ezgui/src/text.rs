// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use graphics;
use graphics::types::Color;
use graphics::{Image, Transformed};
use GfxCtx;

const TEXT_FG_COLOR: Color = [0.0, 0.0, 0.0, 1.0];
const TEXT_HIGHLIGHT_COLOR: Color = [1.0, 0.0, 0.0, 0.5];
const TEXT_BG_COLOR: Color = [0.0, 1.0, 0.0, 0.5];

const FONT_SIZE: u32 = 24;
// TODO this is a hack, need a glyphs.height() method as well!
const END_OF_LINE_CURSOR_WIDTH: f64 = 20.0;
const LINE_HEIGHT: f64 = 22.0;

// TODO I kind of want general HTMLish markup options here -- bold, italic, underline, color, etc
pub struct TextOSD {
    pub(crate) lines: Vec<String>,
    // (Line, character) indices
    pub(crate) highlight_char: Option<(usize, usize)>,
}

impl TextOSD {
    pub fn new() -> TextOSD {
        TextOSD {
            lines: Vec::new(),
            highlight_char: None,
        }
    }

    pub fn pad_if_nonempty(&mut self) {
        if !self.lines.is_empty() {
            self.lines.push("".to_string());
        }
    }

    pub fn add_line(&mut self, line: String) {
        self.lines.push(line);
    }

    pub fn add_line_with_cursor(&mut self, line: String, cursor: usize) {
        assert!(self.highlight_char.is_none());
        // The cursor could be at the end of the line
        assert!(cursor <= line.len());
        self.highlight_char = Some((self.lines.len(), cursor));
        self.lines.push(line);
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

pub fn draw_text_bubble(
    g: &mut GfxCtx,
    lines: &[String],
    (x1, y1): (f64, f64),
    highlight_char: Option<(usize, usize)>,
) {
    let (width, height) = dims(g, lines);
    graphics::Rectangle::new(TEXT_BG_COLOR).draw(
        [x1, y1, width, height],
        &g.orig_ctx.draw_state,
        g.orig_ctx.transform,
        g.gfx,
    );

    let fg_text = Image::new_color(TEXT_FG_COLOR);
    let mut y = y1 + LINE_HEIGHT;
    for (line_idx, line) in lines.iter().enumerate() {
        let mut x = x1;
        for (char_idx, ch) in line.chars().enumerate() {
            if let Ok(draw_ch) = g.glyphs.character(FONT_SIZE, ch) {
                let char_ctx = g
                    .orig_ctx
                    .transform
                    .trans(x + draw_ch.left(), y - draw_ch.top());
                if Some((line_idx, char_idx)) == highlight_char {
                    graphics::Rectangle::new(TEXT_HIGHLIGHT_COLOR).draw(
                        [0.0, 0.0, draw_ch.width(), LINE_HEIGHT],
                        &g.orig_ctx.draw_state,
                        char_ctx,
                        g.gfx,
                    );
                }
                fg_text.draw(draw_ch.texture, &g.orig_ctx.draw_state, char_ctx, g.gfx);
                x += draw_ch.width();
            } else {
                panic!("Couldn't get glyph for {}", ch);
            }
        }
        if Some((line_idx, line.len())) == highlight_char {
            graphics::Rectangle::new(TEXT_HIGHLIGHT_COLOR).draw(
                [x, y - LINE_HEIGHT, END_OF_LINE_CURSOR_WIDTH, LINE_HEIGHT],
                &g.orig_ctx.draw_state,
                g.orig_ctx.transform,
                g.gfx,
            );
        }
        y += LINE_HEIGHT;
    }
}

pub fn dims(g: &mut GfxCtx, lines: &[String]) -> (f64, f64) {
    let longest_line = lines.iter().max_by_key(|l| l.len()).unwrap();
    let width = g.glyphs.width(FONT_SIZE, longest_line).unwrap();
    let height = (lines.len() as f64) * LINE_HEIGHT;
    (width, height)
}
