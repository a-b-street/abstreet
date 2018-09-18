// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use graphics;
use graphics::types::Color;
use graphics::{Image, Transformed};
use GfxCtx;

const TEXT_FG_COLOR: Color = [0.0, 0.0, 0.0, 1.0];
const TEXT_BG_COLOR: Color = [0.0, 1.0, 0.0, 0.5];

const FONT_SIZE: u32 = 24;
// TODO this is a hack, need a glyphs.height() method as well!
const LINE_HEIGHT: f64 = 22.0;

pub fn draw_text_bubble(g: &mut GfxCtx, lines: &[String], (x1, y1): (f64, f64)) {
    let (width, height) = dims(g, lines);
    let tooltip = graphics::Rectangle::new(TEXT_BG_COLOR);
    tooltip.draw(
        [x1, y1, width, height],
        &g.orig_ctx.draw_state,
        g.orig_ctx.transform,
        g.gfx,
    );

    let text = Image::new_color(TEXT_FG_COLOR);
    let mut y = y1 + LINE_HEIGHT;
    for line in lines.iter() {
        let mut x = x1;
        for ch in line.chars() {
            if let Ok(draw_ch) = g.glyphs.character(FONT_SIZE, ch) {
                text.draw(
                    draw_ch.texture,
                    &g.orig_ctx.draw_state,
                    g.orig_ctx
                        .transform
                        .trans(x + draw_ch.left(), y - draw_ch.top()),
                    g.gfx,
                );
                x += draw_ch.width();
            }
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
